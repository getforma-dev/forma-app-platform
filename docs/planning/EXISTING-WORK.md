# Existing work audit — 2026-10-28

This audit read the local Forma work board and existing donor manifests/remotes. It changed no existing tickets, repositories, deployments, or application code. Status is evidence as read on this date, not a promise about future completion.

## Finding

There is no complete platform v2 implementation journey on the local board. The existing material consists of a delivered platform/demo design, an explicit first engineering slice, framework follow-ups, authentication preparation, and older donor migration tickets. Use the application-owned `Tickets/` backlog with `APP-NNN` identifiers and cross-link the existing `FORMA-NNN` work; do not duplicate its implementation ownership or mark it completed because the framework released 3.2.

The local board is explicitly **not part of any repository**. Its authoritative status is the ticket file/folder; `TRACKER.md` is generated. Existing status flow is Backlog → Selected → In progress → In review → Done, with Blocked separate. The journey convention preserves shared planning/evidence in `journeys/`, while ticket files stay in their status folders. This application adopts that same local `Tickets/` folder pattern at the user's request: ticket files and their status folders are authoritative, and generated indexes are read-only views. GitHub hosts source only; do not create parallel GitHub issues or a Notion board. App identifiers are `APP-NNN`, without coupling to the global FORMA numbering/generator. The global board now links this app through its GenUI platform journey; that pointer does not create a second authoritative status copy.

## Team handoff

The user assigns the application to the **GenUI team** and upstream blockers to the **FormaStack team**. FormaStack coordinates the existing FORMA-232/246/247/248 work, including fAuth ownership. GenUI owns the app-side adapters, acceptance and integration. Both teams should proceed in parallel: an unresolved upstream item blocks only the dependent acceptance gate, with the exact required evidence recorded. Snapshot status and assignee fields are historical; the active upstream board carries the subsequent team assignments.

## Current dispositions

| Existing ticket | Status read | Relation to the new app | Recommended treatment |
| --- | --- | --- | --- |
| FORMA-232 — configurable app size budgets | Backlog, P1 | Design assumes it done, but no shipped override API is established by this ticket. Historical 3.0 canvas has only 426 B headroom. | Link as external framework dependency. First app sprint must measure actual 3.2 route and document current limits. Start the bounded slice under shipped limits; if it fails after bounded safe optimization, that dependent task blocks on the framework work. Do not invent syntax or silently increase limits. |
| FORMA-246 — native composition conformance fixture | Backlog, P2 | Historical four-kind canvas builds/renders under 3.0. Browser adoption of the exact fixture remains unverified. | Link as external conformance dependency. Add app-owned 3.2 conformance/adoption/size verification early; this does not close the framework ticket. Escalate actual admission regressions to owning framework work. |
| FORMA-247 — fAuth hosted demo readiness | Backlog, P2 | fAuth deploy at `b750ca53`, Northstar/canary tenant provisioning, registered OAuth client row. Quiet branded login chosen; optional passkey rejected. | Reuse this ticket for fAuth deployment/provisioning, and create distinct app tickets for the platform OAuth/session/authorization adapter and integration acceptance. Hosted demo gate depends on evidence from 247. Synthetic local auth fixture may support early engineering only. |
| FORMA-248 — fAuth signing key custody | Backlog, P2; depends on 247 | Explicitly required before any real organization signs in. Current/next JWKS key rotation and managed key custody. | External real-organization/pilot gate. App ticket should verify unknown-key behavior and rotation integration; do not duplicate the fAuth implementation. |
| FORMA-159–161 — Generative UI donor migration | Backlog | Tier/grants, browser shells, policy and core 3.0 migration for existing `forma-generative-ui` markup host. | Historical donor work, not prerequisites for a clean typed-definition platform. Record reuse decisions instead of inheriting its old API or full migration programme. |
| FORMA-162–163 — canvas library donor migration | Backlog | Old `forma-genui-runtime` peer/core 2.x host, trusted HTML and tier/grants migration. | Reference when reviewing individual reusable canvas primitives. Do not install the old widget-host path or inherit its HTML representation. Port a bounded primitive only after 3.2 acceptance is defined. |
| FORMA-237–242 — markup representation additions/release | Backlog in board; historical release title is stale | Platform foundation review explicitly defers this set under typed AppDefinition to a custom-widget tier. | Exclude from launch blockers. A future customer-driven custom-widget track can rebaseline it; do not mark all done merely because a release called 3.1 happened. |
| FORMA-294 — competitor research and clientTemplates | Done; published stable 3.1 | Separate compiler/runtime experiment, default-off clientTemplates; adoption path remains distinct. | Historical completed framework evidence. Not proof of platform behavior or exact app fixture adoption, not a reason to enable clientTemplates by default. |
| FORMA-301 — router and Forma 3.2 | Done | Stable 3.2.0 published 2026-10-27 at source `fc76ea8531f0e3a85d0224087384e41f52040e3c`, five npm + three Rust packages. | Use 3.2 as requested baseline, with app-specific tests. This supersedes old handoffs calling 301 Backlog. |
| FORMA-253 — FMIR claims measurements | Done | Existing evidence contradicts raw-FMIR semantic retrieval/clustering claims; supports typed metadata/spec retrieval direction. | Link when writing positioning/evaluation tasks. Avoid a new benchmark detour before the app slice. |
| FORMA-355; 331–364 bug/debug journeys | Backlog | Separate optional native route splitting, defects and diagnostics planning. | Reassess only defects applicable to this application's tests. Do not make the entire framework backlog a prerequisite or reopen its release. |

## Authoritative handoff and first slice

The later stable 3.1 handoff points to delivered design v1 at `e279953`, tag `task/forma-generative-app-platform-v2/v1`, and explicitly says:

1. Use typed `AppDefinition` and trusted first-party renderers.
2. Use immutable Parquet/DuckDB sources plus PostgreSQL for work, records, publication and audit.
3. Admit one definition → authorize/query → show a result and receipt → execute an internal command with transactional outcome/audit → reopen it.
4. Prove duplicate submission and version-conflict behavior.
5. Add bounded XLSX and one ported 3.x component before polishing six demo acts.

Those decisions persist. Only the requested framework baseline moves to verified 3.2.0. The original README/readiness text describing an untouched skeleton is older than the delivered design and intake decisions.

## Donor repository inventory

| Local directory | Remote read | Actual manifest baseline | Use |
| --- | --- | --- | --- |
| `FormaBuiltApps/forma-generative-ui` | `https://github.com/getforma-dev/forma-generative-ui.git`; vector-studio upstream with push disabled | `@getforma/core` 2.0.1; build ^0.2.0 | Reference/individual source donor only; not an existing 3.2 platform workspace. |
| `FormaBuiltApps/forma-genui-runtime` | `https://github.com/getforma-dev/forma-genui-runtime.git` | optional core peer ^2.0.0; dev core 2.0.1; private 0.1.0 | Candidate geometry/canvas primitives. Isolate and test ports without adopting old HTML widget host. |
| `FormaBuiltApps/forma-builder` | Local Git repository; `git remote -v` returned no remotes | Express app with bundled `formajs-0.1.0.tgz` | Historical builder; not the platform baseline. |

No Git status was needed because these repositories were read only. No credentials or environment files were inspected/copied.

## Preserved upstream snapshots

[Snapshot index](../../Tickets/evidence/upstream/README.md) preserves FORMA-232, 246, 247, 248, 294 and 301 byte-for-byte, with source paths and SHA-256 in its manifest. These reference copies are never active app tickets.

## Sources

Paths below are local audit provenance; a portable repository should retain relevant excerpts/snapshots and point to its own copied design files rather than depend on these absolute paths at runtime.

- `/Users/victorvillacis/dev/Forma/Tickets/README.md`: workflow, current 3.2 closeout, platform foundation review, journey index.
- `/Users/victorvillacis/dev/Forma/Tickets/TRACKER.md`: related donor and platform dependency entries.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-232.md`: proposed behavior, conditional framework dependency and unchanged status.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-246.md`: actual historical fixture evidence and uncompleted browser acceptance.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-247.md`: identity deployment/provisioning, PKCE client row, no-passkey ruling.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-248.md`: real-organization gate and key custody.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-159.md` through `FORMA-163.md`: donor ownership and old migration scope.
- `/Users/victorvillacis/dev/Forma/Tickets/0-backlog/FORMA-237.md` through `FORMA-242.md`: typed-definition deferral log.
- `/Users/victorvillacis/dev/Forma/Tickets/4-done/FORMA-294.md`: completed compiler scope and published 3.1.
- `/Users/victorvillacis/dev/Forma/Tickets/4-done/FORMA-301.md`: final stable 3.2 disposition at top; historical unchecked sections do not reverse closure.
- `/Users/victorvillacis/dev/Forma/Tickets/4-done/FORMA-253.md`: claims measurement outcome.
- `/Users/victorvillacis/dev/Forma/Tickets/HANDOFF-CODEX-2026-10-25-FORMA294-GENUI.md` and `HANDOFF-CODEX-2026-10-26-STABLE-3.1.md`: explicit platform starting point and unimplemented dependencies.
- `/Users/victorvillacis/dev/Forma/Tickets/journeys/getforma-developer-launch/README.md`: existing journey conventions.

## Limits

This agent audited local ticket ownership and handoffs. It did not freshly prove upstream publication bytes, verify fAuth deployment, execute app tests, or search every remote tracker. The lead's separate GitHub/Notion search and framework baseline audit supply those discovery scopes. Never describe this as proof that no tickets exist anywhere.
