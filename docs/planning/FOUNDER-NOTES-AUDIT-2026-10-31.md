# Founder-notes audit — approved handoff moves

Date: 2026-10-31. Scope: the tracked files in `forma-pitch-mvp`, inspected during the pickup of [the October 31 handoff](https://github.com/getforma-dev/forma-app-platform/blob/93df32175794e856f4aa02055dc5d75b0750c7aa/docs/planning/HANDOFF-2026-10-31.md). Victor reviewed the list and approved **only the two session handoff moves**, preserving their instructions and references. Both originals now live byte-for-byte under the ignored `founder-notes/handoffs/` directory; their tracked removals are part of this change. The pitch-script split and cross-team extraction below remain proposals and were not performed.

The audit inspected the tracked-path inventory, candidate contents and incoming references. It distinguishes personal/session material from the app, design, operating procedures and required evidence. It is not a credential or full-history scan. Files with “notes”, “founder” or “handoff” in their names are not automatically private scratch.

## Approved moves and remaining proposals

The repository-root `founder-notes/` directory is now ignored with `/founder-notes/` in `.gitignore`. Both original handoffs are preserved locally. Private copies are outside `docs/`, where the planning checker scans Markdown regardless of Git tracking. Current procedures are in [DEVELOPMENT](../DEVELOPMENT.md); the maintained [continuation record](CONTINUATION-2026-10-31.md) owns this pickup's results and remaining gates.

| Tracked source | Proposed private destination | Reason and prerequisite |
| --- | --- | --- |
| [CLAUDE-HANDOFF.md at its retained revision](https://github.com/getforma-dev/forma-app-platform/blob/93df32175794e856f4aa02055dc5d75b0750c7aa/CLAUDE-HANDOFF.md) | `founder-notes/handoffs/CLAUDE-HANDOFF-2026-10-29.md` | **Approved and moved.** Historical chat checkpoints, session identifiers, local scratch locations and superseded pickup instructions. Current instructions remain in the developer guide, continuation record, ADRs and tickets. Historical links now use the full immutable pre-move Git revision. |
| [HANDOFF-2026-10-31.md at its retained revision](https://github.com/getforma-dev/forma-app-platform/blob/93df32175794e856f4aa02055dc5d75b0750c7aa/docs/planning/HANDOFF-2026-10-31.md) | `founder-notes/handoffs/HANDOFF-2026-10-31.md` | **Approved and moved.** Session continuation checklist with machine state and branch handoffs. The maintained continuation record carries the current results, remaining work and deployment gate. |
| Founder/investor portions of [PITCH-SCRIPT.md](../pitch/PITCH-SCRIPT.md) | `founder-notes/pitch/investor-script.md` | Mixed document: personal story, market/competition, fundraising ask and investor Q&A are founder material. **Split rather than move the whole file.** Keep the live-demo procedure (section 5), verified technical claims and rehearsal disclosures tracked; [RECORDING.md](../pitch/RECORDING.md) and APP-114 depend on them. |
| Session-transfer paragraph in [CROSS-TEAM-HANDOFF.md](CROSS-TEAM-HANDOFF.md), “Claude transfer and current coordination” | `founder-notes/handoffs/team-session-context-2026-10-29.md` | Optional narrow extraction of chat IDs and session-routing details. Keep dependency ownership, accepted handbacks, deployment boundaries and decisions in the tracked cross-team record. Do not remove the whole handoff. |

No additional standalone personal scratch file was identified in the tracked-path inventory. Application notes code, sample notes fixtures, development launch configuration and synthetic campaign reports are not founder notes.

## References to preserve

These are the incoming references found before the current documentation refresh. Recheck them immediately before any approved move; other work in this pickup changes the README and guide.

| Candidate | Incoming references or obligations |
| --- | --- |
| `CLAUDE-HANDOFF.md` | README's October 29 transfer link; [ADR-005](../architecture/ADR-005-PHASE-1-SEQUENCING.md) ownership link; [ADR-010](../architecture/ADR-010-NOV-2-MVP-CUT.md) dated-summary reference; [PHASE-2-HANDOFF](PHASE-2-HANDOFF.md) transfer link. [MVP-NOV-2-PLAN](MVP-NOV-2-PLAN.md) and the [ADR-008 review](../../evidence/APP-006/adr-008-restructure-review.md) also name it as historical evidence. Preserve their meaning instead of making CI depend on an ignored local file. |
| `HANDOFF-2026-10-31.md` | No incoming tracked path references were found before this audit; this audit now links it, and Victor's pickup request names it. Its outstanding-work and authorization constraints must survive the move. |
| `PITCH-SCRIPT.md` | [GUIDE](../pitch/GUIDE.md) and [RECORDING](../pitch/RECORDING.md) link it. [APP-114](../../Tickets/0-backlog/APP-114.md) uses Appendix B as the marketing claim boundary. [APP-096](../../Tickets/0-backlog/APP-096.md) records its rehearsal history. Retain or repoint these to the tracked demo and claims material after a split. |
| `CROSS-TEAM-HANDOFF.md` | The board generator links it from `Tickets/BLOCKERS.md`; planning docs, APP-074 and upstream evidence link it too. Its path and operational content remain tracked. |

## Keep tracked

- [Founder disposition, October 29](../../evidence/APP-006/founder-disposition-2026-10-29.md): the actual user decision behind ADR-008 and the hosted-identity deferral, not a personal diary.
- [DEV-WORKFLOW-INVENTORY](DEV-WORKFLOW-INVENTORY.md): measured evidence cited by ADR-012 and APP-125–128/131. It includes historical machine observations because those explain the workflow changes.
- [PHASE-2-HANDOFF](PHASE-2-HANDOFF.md) and the operational parts of [CROSS-TEAM-HANDOFF](CROSS-TEAM-HANDOFF.md): phase acceptance, dependency ownership and missing evidence.
- [Northstar BUILD-HANDOFF](../../design/forma-northstar-demo/BUILD-HANDOFF.md), all frozen `design/**/notes.md` files, design manifests and source packets: design provenance. Moving these would change the retained design record.
- [Restore-trial author notes](../../evidence/APP-007/restore-trial-author-notes.md), managed-entry handoffs and upstream handback files under `evidence/`: implementation, refusal, qualification and review evidence. Their filenames do not make them expendable.
- [RECORDING](../pitch/RECORDING.md), [SCENARIOS](../pitch/SCENARIOS.md), fixture CSV/PDF files and the live-demo part of the pitch script: reproducible demonstration procedures and synthetic inputs.
- `.claude/launch.json`, editor tasks, AGENTS.md and contributor instructions: development configuration and procedures.

## Exclusion and verification

Only the two approved handoffs were moved. The verification for this disposition is:

1. Both local copies were compared byte-for-byte with their files at `93df32175794e856f4aa02055dc5d75b0750c7aa`. Historical links in ADR-005, ADR-010, PHASE-2-HANDOFF and APP-006/008 now bind to that revision. Historical review quotations remain unchanged.
2. `/founder-notes/` is in `.gitignore`; only the two agreed source paths are removed. The local copies remain, and the investor narrative and cross-team handoff remain tracked.
3. All three existing Docker contexts (`.dockerignore`, `app/qualification/Dockerfile.dockerignore`, `app/operator/Dockerfile.dockerignore`) start with `**` and admit explicit inputs. None admits `founder-notes/`; preserve that allowlist. Do not add private files to image-copy or CI upload inputs.
4. `Tickets/_tools/board.py` scans README, AGENTS, CONTRIBUTING, `docs/`, `Tickets/`, `fixtures/` and `evidence/`; a root `founder-notes/` is outside that walk. Validate the board from a clean tracked checkout so an ignored local file cannot accidentally satisfy a broken reference. GitHub checkout receives only tracked files.
5. Moving and untracking removes material from future checkouts, not from Git history. This proposal does not rewrite history or claim that already-public material becomes private retroactively.

The audit and the two approved moves are complete. Further splits or moves need Victor's selection. No AWS operation, instance restart or credential read was performed for this audit; no handoff content was deleted.
