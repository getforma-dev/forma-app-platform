# Delivery roadmap

The first deliverable is this organized workspace. The implementation sequence below turns the delivered design into small, checkable outcomes. GenUI owns the app; FormaStack owns upstream framework/authentication blockers. A dependency blocks its consumer, not every ticket in the sprint.

| Stage | Sprints | Outcome | Exit gate |
|---|---|---|---|
| A — Foundations and engineering spike | S01–S02 | Current contracts, native rendering and confined data/parser proofs, followed by one durable authorized workflow | Admitted definition → authorized query → receipt → explicit internal command → atomic outcome/audit → reopen. Replay, conflict and race evidence. Upstream gates cleared before dependent acceptance |
| B — Deterministic app platform | S03–S05 | Real imports, correct query execution, accessible canvas and saved work | CSV and XLSX through actual inspected imports; exact two-domain arithmetic; spatial and focused workflows; durable forms and reopened work |
| C — Creation and governance | S06–S08 | Direct editing, preview, publication, version adoption, retrieval and bounded AI | Both app shapes authored through general controls; exact approvals; genuine generation admitted with evidence; unsupported intents abstain |
| D — Shared two-app demonstration | S09–S10 | Safe sharing, revocation, export, operational rehearsal and the complete six acts | S01–S07 pass on one unchanged platform build; colleague continuation and independent reuse; no per-example patch; restored data and failure handling proven |
| E — Managed pilot readiness | S11 | Hosted isolation, supply chain, audit, support and data lifecycle | Environment and operating decisions recorded; real-identity key custody blocker resolved; deployable controls ready for qualification |
| E — Managed pilot readiness | S12 | Measured load, recovery, security, runbooks and one recurring customer workflow | Real-data entry only after all readiness controls pass; then measure recurring use, second-person reuse and maintenance effort |

Use the [sprint plan](../../Tickets/SPRINTS.md) and [ordered tracker](../../Tickets/TRACKER.md) for exact work and dependencies. Each sprint is a scope batch; no dates or team velocity have been assumed. Size estimates are planning aids, not promises. Split an item before starting if it will exceed five focused implementation days or cannot yield one reviewable result.

## Working rules

- Start one ready primary GenUI ticket at a time. Parallelize independent contracts, fixtures, parser/query work and upstream FormaStack work when the dependency graph permits.
- End each ticket with evidence; end each stage with a demonstration or measured gate. A passing planned-coverage check is not a runtime test.
- Use the same tenant, permission, source, record and version semantics in the spike and the full product. Reduced scale does not waive authority boundaries.
- Do not treat a synthetic provider, seeded app or prerecorded screen as real generation. They can support early development only with an explicit label.
- Keep app code in this repository and upstream fixes in their owners. Donor canvas repositories are reference material; copy selected code only through a porting ticket with license and behavior review.
- Recheck release evidence when an upstream blocker lands. GenUI must rerun its consuming check before moving the blocked ticket to Selected.

## Beyond the first release

The [deferred scope](DEFERRED.md) records candidate follow-ups. It has no launch dependency or committed sprint. The design's first-release promise is a managed company app platform; it does not imply arbitrary code, live connectors, external transactions or a public marketplace.
