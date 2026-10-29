# APP-027 board split review (Phase 2 plan Task 0.2)

Date: 2026-10-29. Reviewer: an independent Claude agent with no part in the edit, read-only, given the plan's Task 0.2, [ADR-009](../../docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md) D9.14 and D9.7 and ADR-008's marker convention, and the uncommitted diff. It was not given the implementer's conclusions. Scope: verbatim clauses, dependency edges, orders, amendment wording, PH02 ranges in the docs, and the board check. This review is a planning-board check only and makes no runtime claim.

## Result

No blockers and no majors. One minor finding, now fixed.

| Check | Result |
| --- | --- |
| Verbatim clauses | Pass. APP-027 keeps "Reload restores permitted personal layout and display settings." APP-081 carries the drawn-connections clause with "(Split from APP-027 by ADR-009 D9.14.)" exactly as the plan quotes it. APP-081's two new lines match the plan character for character and carry no marker. APP-027's other two lines are unchanged. The acceptance sentences of APP-024, APP-026 and APP-031 are untouched; only Log lines were added. |
| Dependency edges | Pass. APP-027 depends on `APP-003, APP-010`. APP-081 has every field the plan specifies: UX, Backlog, P1, Feature, S05, PH02, Order 34, Size M, `C03, C06, R7, S01`, depends on `APP-025, APP-027`. No other ticket's `Depends on` changed. APP-028–031 stay at orders 30–33. |
| Orders | Pass. Every ticket formerly at order n ≥ 34 is now at n + 1, each with exactly one changed line, the `| Order |` row. The 81 orders are unique and contiguous from 1 to 81. |
| Amendment wording | Pass after the fix below. The APP-026, APP-031, APP-027 and APP-081 Log entries are dated and faithful to D9.14. APP-027 §What changes drops "actual evidence connections"; APP-081 §What changes carries it. APP-027 also gains a pointer sentence to APP-081, which the plan did not ask for and which does no harm. |
| Doc ranges | Pass. The edits are `Tickets/plan.json` PH02 deliverable 3 and exit criterion 4, the QUALITY-GATES PH02 row, the PHASE-2-HANDOFF APP-019/APP-027 row, and ROADMAP's PH02 row (18) and total (81 tickets). The generated views are current. No other active doc enumerates PH02 tickets stale. The 17-ticket figures left in ADR-009 and in evidence files are records. |
| Board check | Pass. `python3 Tickets/_tools/board.py check` printed `OK: 81 tickets; 6 phases; 12 sprints; 80 requirements; design digests and active links verified.` |

## Findings and dispositions

- **Minor (fixed): the APP-024 Log was narrower than D9.7.** The amendment named only the third acceptance sentence. D9.7's cache paragraph covers APP-024's cache sentences in the plural, and the fourth sentence's "cached delivery" and "cached private results" are result-cache wording too. The fix rewords the Log entry to cover the cache wording of both the third and fourth sentences. It also lists D9.7's three proof points: reauthorization before a cached source reaches an engine, the uniform 404 on a foreign or revoked receipt lookup, and no engine slot reserved for inactive or pre-access sessions. The acceptance sentences themselves are unchanged.
- **Note (no change): exit criterion 3 does not list APP-081.** PH02 exit criterion 3, "(APP-027–030)", leaves out APP-081 even though APP-081's third line includes a keyboard and focused-view check. The plan limits the `plan.json` edit to deliverable 3 and exit criterion 4, so the criterion is left as it is. Exit criterion 4 and the quality-gate row both include APP-081.
