# Forma app platform work board

Team: **GenUI**. Follow the [phase plan](PHASES.md). Upstream blockers belong to **FormaStack**; see the [blocker register](BLOCKERS.md).

Generated from ticket Markdown. Edit the tickets, then run `python3 Tickets/_tools/board.py render`.

| Order | Ticket | Phase | Sprint | Category | Priority | Size | Status | Depends on |
|---|---|---|---|---|---|---|---|---|
| 1 | [APP-001: Record the released Forma 3.2 baseline and amend design assumptions](4-done/APP-001.md) | [PH01](phases/PH01.md) | S01 | FND | P0 | M | Done | None |
| 2 | [APP-002: Freeze implementation contracts and unresolved decisions](4-done/APP-002.md) | [PH01](phases/PH01.md) | S01 | FND | P0 | M | Done | APP-001 |
| 3 | [APP-003: Qualify a 3.2 native canvas with the full required primitive library](4-done/APP-003.md) | [PH01](phases/PH01.md) | S01 | UX | P0 | M | Done | APP-001 |
| 4 | [APP-004: Qualify the WASM CSV/XLSX parser boundary](4-done/APP-004.md) | [PH01](phases/PH01.md) | S01 | ING | P0 | M | Done | APP-002 |
| 5 | [APP-005: Measure the cold million-row DuckDB path and confinement](4-done/APP-005.md) | [PH01](phases/PH01.md) | S01 | QRY | P0 | M | Done | APP-002 |
| 6 | [APP-006: Revalidate fAuth contract and integration prerequisites](4-done/APP-006.md) | [PH01](phases/PH01.md) | S01 | IAM | P0 | M | Done | APP-001 |
| 7 | [APP-007: Scaffold reproducible browser/Rust workspace and one binary's role interfaces](4-done/APP-007.md) | [PH01](phases/PH01.md) | S02 | FND | P0 | L | Done | APP-001, APP-002 |
| 8 | [APP-008: Establish tenant-bearing schema, database roles and deny-by-default authorization](4-done/APP-008.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-007 |
| 9 | [APP-073: Establish bounded invite and trial admission with owner-issued codes](4-done/APP-073.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008 |
| 10 | [APP-009: Implement fAuth-backed session lifecycle](4-done/APP-009.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008, APP-073 |
| 11 | [APP-074: Qualify invite-only signup and public beta entry locally](4-done/APP-074.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-006, APP-009, APP-073 |
| 12 | [APP-010: Persist and admit a minimal typed definition and work binding](4-done/APP-010.md) | [PH01](phases/PH01.md) | S02 | FND | P0 | L | Done | APP-002, APP-008 |
| 13 | [APP-011: Package reproducible synthetic sources and expected outcomes](4-done/APP-011.md) | [PH01](phases/PH01.md) | S02 | VAL | P0 | M | Done | APP-002, APP-007 |
| 14 | [APP-012: Deliver a minimal authorized query with receipt](4-done/APP-012.md) | [PH01](phases/PH01.md) | S02 | QRY | P0 | L | Done | APP-003, APP-005, APP-009, APP-010, APP-011 |
| 15 | [APP-013: Commit one declared internal record and recover outcome](4-done/APP-013.md) | [PH01](phases/PH01.md) | S02 | REC | P0 | L | Done | APP-012 |
| 16 | [APP-014: Prove command retries, conflicts and authorization race ordering](4-done/APP-014.md) | [PH01](phases/PH01.md) | S02 | REC | P0 | M | Done | APP-013 |
| 17 | [APP-015: Implement durable jobs, leases and cancellation](0-backlog/APP-015.md) | [PH02](phases/PH02.md) | S03 | OPS | P0 | L | Backlog | APP-008, APP-009, APP-073 |
| 18 | [APP-016: Store original and normalized immutable revisions](0-backlog/APP-016.md) | [PH02](phases/PH02.md) | S03 | ING | P0 | L | Backlog | APP-008, APP-010 |
| 19 | [APP-017: Build CSV and XLSX import jobs](0-backlog/APP-017.md) | [PH02](phases/PH02.md) | S03 | ING | P1 | L | Backlog | APP-004, APP-015, APP-016 |
| 20 | [APP-018: Confirm source interpretation and compatibility](0-backlog/APP-018.md) | [PH02](phases/PH02.md) | S03 | ING | P1 | L | Backlog | APP-017 |
| 21 | [APP-019: Implement accessible drop/upload source cards](0-backlog/APP-019.md) | [PH02](phases/PH02.md) | S03 | UX | P1 | L | Backlog | APP-003, APP-017, APP-018 |
| 22 | [APP-020: Complete the typed query grammar and admission checks](0-backlog/APP-020.md) | [PH02](phases/PH02.md) | S04 | QRY | P0 | L | Backlog | APP-005, APP-012, APP-018 |
| 23 | [APP-021: Enforce exact arithmetic, units, windows and result-scope semantics](0-backlog/APP-021.md) | [PH02](phases/PH02.md) | S04 | QRY | P0 | L | Backlog | APP-020 |
| 24 | [APP-022: Register the bounded domain operators](0-backlog/APP-022.md) | [PH02](phases/PH02.md) | S04 | QRY | P1 | L | Backlog | APP-020, APP-021 |
| 25 | [APP-023: Complete receipt provenance and record snapshots](0-backlog/APP-023.md) | [PH02](phases/PH02.md) | S04 | QRY | P0 | L | Backlog | APP-013, APP-014, APP-020, APP-021, APP-022 |
| 26 | [APP-024: Enforce query admission, deadlines and cache authority](0-backlog/APP-024.md) | [PH02](phases/PH02.md) | S04 | QRY | P0 | L | Backlog | APP-015, APP-020, APP-021 |
| 27 | [APP-025: Link views by exact context and filter epoch](0-backlog/APP-025.md) | [PH02](phases/PH02.md) | S04 | UX | P1 | L | Backlog | APP-023, APP-024 |
| 28 | [APP-026: Company home, spaces, catalogs, click/slash navigation](0-backlog/APP-026.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | L | Backlog | APP-009, APP-010 |
| 29 | [APP-027: Port measured canvas interaction and structured alternative](0-backlog/APP-027.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | L | Backlog | APP-003, APP-010 |
| 30 | [APP-028: Complete the trusted native renderer catalog](0-backlog/APP-028.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | L | Backlog | APP-027 |
| 31 | [APP-029: Verify accessible component states and equivalent focused workflows](0-backlog/APP-029.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | M | Backlog | APP-028 |
| 32 | [APP-030: Generalize forms and commands without weakening transaction invariants](0-backlog/APP-030.md) | [PH02](phases/PH02.md) | S05 | REC | P0 | L | Backlog | APP-013, APP-014, APP-028, APP-029 |
| 33 | [APP-031: Separate saved work from personal presentation](0-backlog/APP-031.md) | [PH02](phases/PH02.md) | S05 | GOV | P1 | L | Backlog | APP-026, APP-030 |
| 34 | [APP-081: Integrate linked views and evidence connections on the canvas](0-backlog/APP-081.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | M | Backlog | APP-025, APP-027 |
| 35 | [APP-032: Complete AppDefinition schema and registry admission](0-backlog/APP-032.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-010, APP-022, APP-028, APP-029, APP-030 |
| 36 | [APP-033: Build direct composition/editor controls](0-backlog/APP-033.md) | [PH03](phases/PH03.md) | S06 | AUT | P1 | L | Backlog | APP-032 |
| 37 | [APP-034: Implement patches, semantic diff and undo revisions](0-backlog/APP-034.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-033 |
| 38 | [APP-035: Isolate preview execution and draft records](0-backlog/APP-035.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-023, APP-030, APP-034 |
| 39 | [APP-036: Prove authoring generality before AI polish](0-backlog/APP-036.md) | [PH03](phases/PH03.md) | S06 | VAL | P0 | M | Backlog | APP-033, APP-035 |
| 40 | [APP-037: Implement versioned publication policy and lifecycle rules](0-backlog/APP-037.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-008, APP-032, APP-034 |
| 41 | [APP-038: Commit exact publications atomically under current policy](0-backlog/APP-038.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-037 |
| 42 | [APP-039: Reviewer inbox over admitted bytes](0-backlog/APP-039.md) | [PH03](phases/PH03.md) | S07 | GOV | P1 | L | Backlog | APP-035, APP-037, APP-038 |
| 43 | [APP-040: Implement explicit work-version adoption and rollback](0-backlog/APP-040.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-031, APP-037, APP-038 |
| 44 | [APP-041: Ownership, app placement, retirement and missing dependencies](0-backlog/APP-041.md) | [PH03](phases/PH03.md) | S07 | GOV | P1 | L | Backlog | APP-026, APP-037, APP-038, APP-040 |
| 45 | [APP-042: Build private compatible catalog retrieval](0-backlog/APP-042.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-037, APP-038, APP-041 |
| 46 | [APP-043: Configure provider/disclosure adapter and budget control](0-backlog/APP-043.md) | [PH03](phases/PH03.md) | S08 | AI | P0 | L | Backlog | APP-015, APP-032 |
| 47 | [APP-044: Compose, adapt and propose changes as durable jobs](0-backlog/APP-044.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-034, APP-042, APP-043 |
| 48 | [APP-045: Enforce the five assistant tool boundaries](0-backlog/APP-045.md) | [PH03](phases/PH03.md) | S08 | AI | P0 | L | Backlog | APP-030, APP-044 |
| 49 | [APP-046: Explain authorized results from receipts](0-backlog/APP-046.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-023, APP-043, APP-045 |
| 50 | [APP-047: Freeze and run the 15-intent evaluation](0-backlog/APP-047.md) | [PH03](phases/PH03.md) | S08 | VAL | P0 | M | Backlog | APP-044, APP-045, APP-046 |
| 51 | [APP-048: Separate app, work, source, record and export grants](0-backlog/APP-048.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-023, APP-031, APP-037, APP-038 |
| 52 | [APP-049: Protect source-dependent saved and derived information](0-backlog/APP-049.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-048 |
| 53 | [APP-050: Deliver and measure revocation end to end](0-backlog/APP-050.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-024, APP-040, APP-048, APP-049 |
| 54 | [APP-051: Export an authorized labeled snapshot](0-backlog/APP-051.md) | [PH04](phases/PH04.md) | S09 | SHR | P1 | L | Backlog | APP-015, APP-048, APP-049, APP-050 |
| 55 | [APP-052: Exercise handoff, independent reuse and all data surfaces](0-backlog/APP-052.md) | [PH04](phases/PH04.md) | S09 | VAL | P0 | M | Backlog | APP-041, APP-050, APP-051 |
| 56 | [APP-053: Prove synthetic restart, restore, deletion and quota recovery](0-backlog/APP-053.md) | [PH04](phases/PH04.md) | S09 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-015, APP-016, APP-024, APP-043, APP-048, APP-049 |
| 57 | [APP-054: Run the complete Northstar journey from imported evidence](0-backlog/APP-054.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-019, APP-025, APP-039, APP-047, APP-052, APP-053 |
| 58 | [APP-055: Create Spend Review through the same live authoring path](0-backlog/APP-055.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-054 |
| 59 | [APP-056: Run failure, security and accessibility release suite](0-backlog/APP-056.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-047, APP-052, APP-053, APP-055 |
| 60 | [APP-057: Package reproducible demo evidence and rehearsal](0-backlog/APP-057.md) | [PH04](phases/PH04.md) | S10 | VAL | P1 | M | Backlog | APP-056 |
| 61 | [APP-058: Record pilot scope and real-identity prerequisites](9-blocked/APP-058.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Blocked | APP-057 |
| 62 | [APP-059: Prepare immutable role-separated hosted infrastructure](0-backlog/APP-059.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-007, APP-008, APP-058 |
| 63 | [APP-060: Authenticate service calls with constrained role identities](0-backlog/APP-060.md) | [PH05](phases/PH05.md) | S11 | IAM | P0 | L | Backlog | APP-059 |
| 64 | [APP-061: Enforce supply-chain and reversible deployment gates](0-backlog/APP-061.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-059, APP-060 |
| 65 | [APP-062: Complete append-only audit chains and immutable anchoring](0-backlog/APP-062.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-037, APP-038, APP-053, APP-059, APP-060 |
| 66 | [APP-063: Control support access and emergency administrative operations](0-backlog/APP-063.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062 |
| 67 | [APP-064: Apply complete retention and source deletion policy](0-backlog/APP-064.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-016, APP-048, APP-049, APP-058, APP-059, APP-060 |
| 68 | [APP-065: Execute reversible deletion holds and complete organization erasure](0-backlog/APP-065.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062, APP-063, APP-064 |
| 69 | [APP-066: Build admin and operator workflows](0-backlog/APP-066.md) | [PH05](phases/PH05.md) | S11 | OPS | P1 | L | Backlog | APP-041, APP-059, APP-060, APP-062, APP-063, APP-064, APP-065 |
| 70 | [APP-067: Instrument SLOs, cost and payload-safe telemetry](0-backlog/APP-067.md) | [PH06](phases/PH06.md) | S12 | OPS | P1 | L | Backlog | APP-059, APP-060, APP-062, APP-063 |
| 71 | [APP-068: Exercise restore and failure drills](0-backlog/APP-068.md) | [PH06](phases/PH06.md) | S12 | OPS | P0 | L | Backlog | APP-061, APP-064, APP-065, APP-067 |
| 72 | [APP-069: Qualify load, noisy neighbor and budgets at declared limits](0-backlog/APP-069.md) | [PH06](phases/PH06.md) | S12 | VAL | P0 | M | Backlog | APP-024, APP-059, APP-060, APP-067 |
| 73 | [APP-070: Independent release security and accessibility review](0-backlog/APP-070.md) | [PH06](phases/PH06.md) | S12 | VAL | P0 | M | Backlog | APP-062, APP-063, APP-064, APP-065, APP-068, APP-069 |
| 74 | [APP-071: Publish operator/run/release/rollback and incident runbooks](0-backlog/APP-071.md) | [PH06](phases/PH06.md) | S12 | OPS | P0 | M | Backlog | APP-066, APP-067, APP-068, APP-070 |
| 75 | [APP-072: Evaluate one real recurring workflow and second-person reuse](0-backlog/APP-072.md) | [PH06](phases/PH06.md) | S12 | VAL | P1 | M | Backlog | APP-058, APP-070, APP-071 |
| 76 | [APP-075: Finish the managed qualification entry, image and supervisor](4-done/APP-075.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008 |
| 77 | [APP-077: Package the operator image and tools](4-done/APP-077.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008 |
| 78 | [APP-078: Prove tenant isolation and role bindings on managed RDS (managed qualification verdict)](2-in-progress/APP-078.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | In progress | APP-075, APP-077 |
| 79 | [APP-076: Run the beta go-live trial on the live deployment](0-backlog/APP-076.md) | [PH01](phases/PH01.md) | S02 | VAL | P0 | L | Backlog | APP-012, APP-013, APP-014, APP-074, APP-078 |
| 80 | [APP-079: Hosted identity acceptance deferred from Phase 1](9-blocked/APP-079.md) | [PH06](phases/PH06.md) | S12 | IAM | P1 | L | Blocked | APP-076 |
| 81 | [APP-080: Qualify tenant races, drift and recovery on managed RDS](0-backlog/APP-080.md) | [PH05](phases/PH05.md) | S11 | OPS | P1 | L | Backlog | APP-077, APP-078 |
| 82 | [APP-082: Prove route-level admission races and provider faults on the local public entry (deferred from APP-074)](4-done/APP-082.md) | [PH07](phases/PH07.md) | S13 | IAM | P1 | M | Done | APP-074 |
| 83 | [APP-083: Prove concurrent redemption through the public beta API on the live deployment (deferred from APP-076)](0-backlog/APP-083.md) | [PH07](phases/PH07.md) | S13 | VAL | P1 | S | Backlog | APP-076 |
| 84 | [APP-084: Close the admission and provider review residuals (APP-073 reviews)](4-done/APP-084.md) | [PH07](phases/PH07.md) | S13 | IAM | P1 | M | Done | APP-073 |
| 85 | [APP-085: Close the definition and publication review residuals (APP-010 review)](2-in-progress/APP-085.md) | [PH07](phases/PH07.md) | S13 | FND | P2 | S | In progress | APP-010 |
| 86 | [APP-086: Close the managed qualification and native campaign residuals (APP-075 and APP-008 reviews)](0-backlog/APP-086.md) | [PH07](phases/PH07.md) | S13 | OPS | P2 | M | Backlog | APP-075, APP-078 |
| 87 | [APP-087: Decide the parser fuel bound and close the parser packaging residuals (APP-017 preparation review)](0-backlog/APP-087.md) | [PH02](phases/PH02.md) | S03 | ING | P1 | M | Backlog | APP-004 |
| 88 | [APP-088: Fold the ADR-009 re-review and contract-edition residuals into the Phase 2 record (APP-015 preparation)](4-done/APP-088.md) | [PH02](phases/PH02.md) | S03 | QRY | P1 | S | Done | None |
| 89 | [APP-089: Harden the container images: base CVEs and effective scan-on-push](0-backlog/APP-089.md) | [PH02](phases/PH02.md) | S03 | OPS | P1 | M | Backlog | APP-075, APP-078 |
| 90 | [APP-090: MVP UI fixes and the designer handoff](0-backlog/APP-090.md) | [PH02](phases/PH02.md) | S03 | UX | P1 | M | Backlog | APP-076, APP-078 |
| 091 | [APP-091: Pitch clone and join plan: canvas, planner and composer on the platform backbone](4-done/APP-091.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | M | Done | APP-012, APP-074 |
| 092 | [APP-092: Canvas in the signed-in workspace](4-done/APP-092.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Done | APP-091 |
| 093 | [APP-093: Composer: describe an app and assemble it from admitted modules](4-done/APP-093.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Done | APP-091, APP-092 |
| 094 | [APP-094: Governed data on the canvas: views with receipts and the record form](4-done/APP-094.md) | [PH08](phases/PH08.md) | S14 | QRY | P0 | L | Done | APP-093 |
| 095 | [APP-095: Share an app with a teammate under policy](4-done/APP-095.md) | [PH08](phases/PH08.md) | S14 | SHR | P0 | M | Done | APP-094 |
| 096 | [APP-096: Deploy and rehearse the pitch on beta.getforma.dev](0-backlog/APP-096.md) | [PH08](phases/PH08.md) | S14 | VAL | P0 | L | Backlog | APP-092, APP-093, APP-094, APP-095 |
| 097 | [APP-097: Post-November 2 revisit: land the pitch and close the deferred work](0-backlog/APP-097.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | L | Backlog | APP-076, APP-078, APP-086, APP-089, APP-090, APP-014, APP-084, APP-096 |
| 098 | [APP-098: Demo shortcuts register: every mock, hard-code and pitch limit behind GENUI_DEMO_MODE](2-in-progress/APP-098.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | M | In progress | APP-091 |
| 099 | [APP-099: Forma as an MCP server: any agent (Claude, OpenAI, others) works through the governed canvas](0-backlog/APP-099.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Backlog | APP-093, APP-095 |
| 100 | [APP-100: Slack: file, approve and get notified in Slack through the governed action path](0-backlog/APP-100.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Backlog | APP-093, APP-095, APP-099 |
| 101 | [APP-101: Instant pick and live transform: prebuilt apps land at once and reshape to the request](4-done/APP-101.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Done | APP-093, APP-094 |
| 102 | [APP-102: Receipt upload and the expense ledger: a photo becomes a reviewed row, every row adds up live](4-done/APP-102.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | M | Done | APP-093, APP-095 |
| 103 | [APP-103: Canvas utility widgets: weather, calculator and clock that a member customizes in words](4-done/APP-103.md) | [PH08](phases/PH08.md) | S14 | AI | P1 | M | Done | APP-094, APP-101 |
| 104 | [APP-104: Canvas library pane, a clean board, and Figma-style controls](4-done/APP-104.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-094, APP-101, APP-103 |
| 105 | [APP-105: Ad hoc pitch work of 2026-10-30: deploy fixes, live operations, canvas additions and what remains](0-backlog/APP-105.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | M | Backlog | APP-096, APP-099, APP-100, APP-101, APP-102, APP-103, APP-104 |
| 106 | [APP-106: Focus mode and Unite: work on a card in a modal, and assemble an app's cards into the whole app](4-done/APP-106.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-104 |
| 107 | [APP-107: Workflow library: the approval processes real companies run, end to end](0-backlog/APP-107.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Backlog | APP-093, APP-095, APP-099, APP-100 |
| 108 | [APP-108: Folders (boxes): file cards with a viewer, real CSV data, and apps built from a folder that trace back to it](4-done/APP-108.md) | [PH08](phases/PH08.md) | S14 | AI | P0 | L | Done | APP-093, APP-095, APP-104, APP-106 |
| 109 | [APP-109: Quick notes: /note saves a note as a card](4-done/APP-109.md) | [PH08](phases/PH08.md) | S14 | UX | P2 | S | Done | APP-103, APP-104 |
| 110 | [APP-110: Pitch scope ledger: what the demo ships, what is deferred, and what needs enterprise refinement](0-backlog/APP-110.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | M | Backlog | APP-098, APP-105 |
| 111 | [APP-111: Pro plan: large documents become a cited brief and a shareable app](0-backlog/APP-111.md) | [PH08](phases/PH08.md) | S14 | AI | P1 | XL | Backlog | APP-108 |
| 112 | [APP-112: Command parity: every slash command has a click path](4-done/APP-112.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | S | Done | APP-104, APP-106, APP-108, APP-109 |
| 113 | [APP-113: Code review of the pitch (2026-10-30): findings, fixes and deferrals](0-backlog/APP-113.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | S | Backlog | APP-099, APP-100, APP-104, APP-107, APP-108 |
| 114 | [APP-114: Landing page for getforma.dev: the product, how it connects, security, and the beta list](0-backlog/APP-114.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Backlog | APP-096, APP-110 |
| 115 | [APP-115: Agent access governance: agent identities, scoped tokens, organization policies, and an admin trace console](0-backlog/APP-115.md) | [PH08](phases/PH08.md) | S14 | GOV | P1 | XL | Backlog | APP-099, APP-100, APP-113 |
| 116 | [APP-116: Timing chip: real step durations on screen for demos and trust](4-done/APP-116.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | S | Done | APP-101, APP-104 |
| 117 | [APP-117: Readable organization addresses: sign-in alias (done), organization in every URL, subdomains](0-backlog/APP-117.md) | [PH08](phases/PH08.md) | S14 | IAM | P1 | L | Backlog | APP-096 |
| 118 | [APP-118: Command system: one registry, argument hints, /commands help, /apps opens the Library](4-done/APP-118.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-104, APP-112 |
| 119 | [APP-119: Commands are verbs, apps are addresses: split the registry, derive app hints from definitions](4-done/APP-119.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-118 |
| 120 | [APP-120: One canvas model: apps (with cards), widgets, arrangements; one grammar; Library = Apps + Widgets](4-done/APP-120.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-119 |
| 121 | [APP-121: User naming: app and card names, addresses, descriptions (owner) and personal aliases (member)](4-done/APP-121.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | L | Done | APP-120 |
| 122 | [APP-122: Two AI jobs: Ask (read-only conversation about the data) and Shape (change the look or view of a card, an app or a widget), separated by where you type](4-done/APP-122.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | APP-121 |
| 123 | [APP-123: Ask and Shape, after the demo: Make it a card, personal cards, @ mentions, /forma, /<folder> words](0-backlog/APP-123.md) | [PH08](phases/PH08.md) | S14 | UX | P2 | M | Backlog | APP-122, APP-097 |
| 124 | [APP-124: Start from your data: drop files, Ask the folder, Make a card, Make an app](0-backlog/APP-124.md) | [PH08](phases/PH08.md) | S14 | UX | P2 | M | Backlog | APP-108, APP-122, APP-123 |
| 125 | [APP-125: Developer workflow step 1: pins, dev profile, per-checkout venvs, no committed symlink](4-done/APP-125.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | None |
| 126 | [APP-126: Developer workflow step 2: the front door (justfile, campaign catalog, VS Code workspace)](4-done/APP-126.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125 |
| 127 | [APP-127: Developer workflow step 3: the beta deploy in Git (ops/beta, $PRIV per checkout, --approved gate)](4-done/APP-127.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | M | Done | APP-126 |
| 128 | [APP-128: Developer workflow step 4: the genui-dev binary under process-compose (Compose PostgreSQL, in-process personas, clean profile)](4-done/APP-128.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | M | Done | APP-125, APP-126 |
| 129 | [APP-129: Contract edition impl-2026-10-31.4: lock what the pitch ships (APP-107 amendment, authoring rules for new content, library)](0-backlog/APP-129.md) | [PH08](phases/PH08.md) | S14 | FND | P1 | S | Backlog | APP-107, APP-113 |
| 130 | [APP-130: Repository and delivery step C/D: one root Cargo workspace, the builder-cache sweep](4-done/APP-130.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126, APP-127, APP-128 |
| 131 | [APP-131: Developer workflow step 5: CI green or quarantined (Foundation, Tenant foundation, the APP-013 campaign)](4-done/APP-131.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126 |
| 132 | [APP-132: Clean slate: a sentence builds a first governed records app (generation refused as not_runnable without a library or sources)](4-done/APP-132.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Done | APP-128 |
| 133 | [APP-133: Repository and delivery: the release record (release.json, RELEASES.md, beta tags) and the module map](4-done/APP-133.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-127, APP-130 |
| 135 | [APP-135: MVP overbuild and hard-code audit: everything that only works because of the seeded library, the demo organization or fixed data](2-in-progress/APP-135.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | In progress | APP-132 |
| 136 | [APP-136: Developer guide from setup through beta delivery](4-done/APP-136.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126, APP-127, APP-128, APP-130, APP-131, APP-133 |
| 137 | [APP-137: One private beta ledger across repository worktrees](4-done/APP-137.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-127 |
| 138 | [APP-138: Forma Canvas — shared Forma brand and application design](4-done/APP-138.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | M | Done | None |
| 139 | [APP-145: Give a new member one authoring shell so the first description can publish](4-done/APP-145.md) | [PH08](phases/PH08.md) | S14 | AUT | P0 | M | Done | APP-093 |
| 140 | [APP-141: Create an app before it reaches the canvas](4-done/APP-141.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Done | APP-093, APP-104, APP-120, APP-145 |
| 141 | [APP-142: The app is a page: focus inside it, fan-out back to cards](4-done/APP-142.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Done | APP-106, APP-120, APP-141 |
| 142 | [APP-144: Open an app as a page and use it there](4-done/APP-144.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Done | APP-142, APP-093 |
| 143 | [APP-143: Ask about the app, the card, or the widget you clicked](4-done/APP-143.md) | [PH08](phases/PH08.md) | S14 | AI | P1 | M | Done | APP-094, APP-142 |
| 144 | [APP-154: Private live app sharing, external guests and connected canvas placement](4-done/APP-154.md) | [PH08](phases/PH08.md) | S14 | SHR | P0 | L | Done | APP-074, APP-095, APP-144 |
| 145 | [APP-139: Guided first-run trial: a new member, an empty canvas, one app, one record, one share](0-backlog/APP-139.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | M | Backlog | APP-074, APP-093, APP-095, APP-145, APP-144, APP-154 |
| 146 | [APP-140: Clean trial twin and panel accounts, with the sandbox kept off AWS](0-backlog/APP-140.md) | [PH08](phases/PH08.md) | S14 | FND | P0 | M | Backlog | APP-139 |
| 147 | [APP-146: Spike — what an app is](4-done/APP-146.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | S | Done | APP-120 |
| 148 | [APP-147: Spike — what a component of an app is](4-done/APP-147.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | S | Done | APP-120 |
| 149 | [APP-148: Spike — what a widget is](4-done/APP-148.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | S | Done | APP-103, APP-120 |
| 150 | [APP-149: Spike — what a box is, and where it sits](4-done/APP-149.md) | [PH08](phases/PH08.md) | S14 | UX | P0 | S | Done | APP-108, APP-120 |
| 151 | [APP-150: Favorite sections, and a way to go to one](0-backlog/APP-150.md) | [PH08](phases/PH08.md) | S14 | UX | P2 | S | Backlog | APP-142 |
| 152 | [APP-151: Middle-click zooms into focus, then Tab moves through the app](0-backlog/APP-151.md) | [PH08](phases/PH08.md) | S14 | UX | P2 | S | Backlog | APP-106, APP-142 |
| 153 | [APP-152: An API bank, and an easy way to add an API](0-backlog/APP-152.md) | [PH08](phases/PH08.md) | S14 | AI | P2 | L | Backlog | APP-093, APP-104 |
| 154 | [APP-153: Spike — why the library pane has four tabs](4-done/APP-153.md) | [PH08](phases/PH08.md) | S14 | UX | P1 | S | Done | APP-112, APP-122 |
| 155 | [APP-155: One coherent app presentation on the canvas and at its own URL](0-backlog/APP-155.md) | [PH09](phases/PH09.md) | S15 | UX | P1 | L | Backlog | APP-139, APP-140, APP-142, APP-144, APP-146, APP-147, APP-154 |
| 156 | [APP-156: Typed buttons that open app components and command forms](0-backlog/APP-156.md) | [PH09](phases/PH09.md) | S15 | AUT | P1 | L | Backlog | APP-155 |
| 157 | [APP-157: Persisted bounded app appearance across all presentations](0-backlog/APP-157.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | M | Backlog | APP-155, APP-122 |
| 158 | [APP-158: Place an admitted records list with authoritative scoped filters](0-backlog/APP-158.md) | [PH09](phases/PH09.md) | S15 | UX | P1 | L | Backlog | APP-144, APP-155, APP-020, APP-025 |
| 159 | [APP-159: Compute bounded authorized record summaries, trends and rankings](0-backlog/APP-159.md) | [PH09](phases/PH09.md) | S15 | QRY | P1 | L | Backlog | APP-158, APP-021, APP-023, APP-024 |
| 160 | [APP-160: Qualify one composed reference app across canvas, page and sharing](0-backlog/APP-160.md) | [PH09](phases/PH09.md) | S15 | UX | P1 | L | Backlog | APP-154, APP-155, APP-156, APP-157, APP-158, APP-159 |
| 161 | [APP-161: Spike — measure idle app costs and decide safe sleep/wake architecture](0-backlog/APP-161.md) | [PH09](phases/PH09.md) | S15 | OPS | P2 | M | Backlog | APP-140, APP-154 |
| 162 | [APP-162: Workspace name/link entry and explicit switch on sign-in](4-done/APP-162.md) | [PH08](phases/PH08.md) | S14 | IAM | P0 | M | Done | APP-009 |
| 163 | [APP-163: Find my workspaces after verified email discovery](9-blocked/APP-163.md) | [PH08](phases/PH08.md) | S14 | IAM | P1 | L | Blocked | APP-162 |
| 164 | [APP-164: Authenticated workspace chooser and safe session switching](0-backlog/APP-164.md) | [PH08](phases/PH08.md) | S14 | IAM | P1 | L | Backlog | APP-162, APP-163 |
| 165 | [APP-165: Deliver workspace-aware membership and shared-app invitations](0-backlog/APP-165.md) | [PH08](phases/PH08.md) | S14 | IAM | P1 | M | Backlog | APP-162, APP-154 |
| 166 | [APP-166: Theme roles so the canvas, an app, a card and a widget can share or split an identity](4-done/APP-166.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | M | Done | APP-138 |
| 167 | [APP-167: Promote merged beta releases through GitHub Actions](4-done/APP-167.md) | [PH08](phases/PH08.md) | S14 | OPS | P0 | L | Done | None |
| 168 | [APP-168: One whole app on the canvas and an Apps, Widgets, Cards Library](4-done/APP-168.md) | [PH09](phases/PH09.md) | S15 | UX | P1 | L | Done | None |
| 169 | [APP-169: Persist artifact classification and retire legacy app placement after compatibility review](4-done/APP-169.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | M | Done | APP-168 |
| 170 | [APP-170: Publish component visibility consistently across app views and recipients](0-backlog/APP-170.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | M | Backlog | APP-168 |
| 171 | [APP-171: Evaluate a desktop icon presentation for App Cards](0-backlog/APP-171.md) | [PH09](phases/PH09.md) | S15 | UX | P3 | S | Backlog | APP-168 |
| 172 | [APP-172: Upgrade GenUI and GateWasm PostgreSQL from 16.13 to 16.15 after the image release](4-done/APP-172.md) | [PH08](phases/PH08.md) | S14 | OPS | P1 | L | Done | APP-167 |
| 173 | [APP-173: Review the November 5 base-image scan findings so beta promotion can continue](4-done/APP-173.md) | [PH08](phases/PH08.md) | S14 | OPS | P0 | S | Done | APP-167 |
| 174 | [APP-174: Detach the Northstar management secret, rotate its key and clean up the invitation input secret](9-blocked/APP-174.md) | [PH07](phases/PH07.md) | S13 | IAM | P1 | S | Blocked | APP-084 |
| 175 | [APP-175: A reviewed operator schema release for migrations 21 and 22](4-done/APP-175.md) | [PH09](phases/PH09.md) | S15 | OPS | P1 | M | Done | APP-084, APP-169, APP-167 |
| 176 | [APP-176: Classify existing ambiguous works with owner review and retire legacy app placement](0-backlog/APP-176.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | M | Backlog | APP-169 |
| 177 | [APP-177: Decide and, if kept, build canvas fences for whole apps](4-done/APP-177.md) | [PH09](phases/PH09.md) | S15 | UX | P3 | S | Done | APP-104 |
| 178 | [APP-178: Prove shared-app access fails closed on organization shutdown, expired invitations and overlapping grants](4-done/APP-178.md) | [PH09](phases/PH09.md) | S15 | SHR | P1 | S | Done | APP-154 |
| 179 | [APP-179: Correct the Shortcuts sheet and guides after whole-app views, and test note privacy](4-done/APP-179.md) | [PH09](phases/PH09.md) | S15 | UX | P2 | S | Done | APP-112, APP-120 |
| 180 | [APP-180: Separate the drafter from the reviewer of an independently reviewed publication](0-backlog/APP-180.md) | [PH07](phases/PH07.md) | S13 | FND | P2 | S | Backlog | APP-085 |
| 181 | [APP-181: Record the real live test charter and the beta notice](0-backlog/APP-181.md) | [PH09](phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-139 |
| 182 | [APP-182: Require and record consent to the beta notice at invitation](0-backlog/APP-182.md) | [PH09](phases/PH09.md) | S15 | FND | P0 | M | Backlog | APP-181 |
| 183 | [APP-183: Organization Admin: see the members and remove one](0-backlog/APP-183.md) | [PH09](phases/PH09.md) | S15 | SHR | P0 | M | Backlog | APP-181 |
| 184 | [APP-184: An app-wide daily cap on model calls, with a plain message](0-backlog/APP-184.md) | [PH09](phases/PH09.md) | S15 | FND | P0 | S | Backlog | APP-181 |
| 185 | [APP-185: Operator back office for admissions after the cutover](0-backlog/APP-185.md) | [PH09](phases/PH09.md) | S15 | OPS | P0 | M | Backlog | APP-181 |
| 186 | [APP-186: Renew the beta and the budget for the test window](0-backlog/APP-186.md) | [PH09](phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-181 |
| 187 | [APP-187: Email alarms and a budget alert](0-backlog/APP-187.md) | [PH09](phases/PH09.md) | S15 | OPS | P1 | S | Backlog | APP-181 |
| 188 | [APP-188: Operations runbook, restore drill and end-of-test purge](0-backlog/APP-188.md) | [PH09](phases/PH09.md) | S15 | OPS | P1 | S | Backlog | APP-185 |
| 190 | [APP-190: Member names that each member sets](0-backlog/APP-190.md) | [PH09](phases/PH09.md) | S15 | SHR | P0 | S | Backlog | APP-181 |
| 191 | [APP-191: Private entries and reviewer-only commands](0-backlog/APP-191.md) | [PH09](phases/PH09.md) | S15 | FND | P0 | L | Backlog | APP-181 |
| 192 | [APP-192: Share with roles and with everyone in the organization](0-backlog/APP-192.md) | [PH09](phases/PH09.md) | S15 | SHR | P0 | M | Backlog | APP-181, APP-191 |
| 193 | [APP-193: Spike: live interaction from general primitives](0-backlog/APP-193.md) | [PH09](phases/PH09.md) | S15 | FND | P2 | S | Backlog | APP-191 |
| 194 | [APP-194: Schema release B: migrations 0023 and 0024](0-backlog/APP-194.md) | [PH09](phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-175, APP-182, APP-183, APP-186, APP-190, APP-191, APP-192 |
| 195 | [APP-195: Refresh herd: bounded waiting at the data pool and jittered live refreshes](0-backlog/APP-195.md) | [PH09](phases/PH09.md) | S15 | FND | P0 | M | Backlog | APP-181 |
| 196 | [APP-189: Go-live: build day, rehearsal, codes and the class](0-backlog/APP-189.md) | [PH09](phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-182, APP-183, APP-184, APP-185, APP-186, APP-187, APP-188, APP-190, APP-191, APP-192, APP-194, APP-195 |
| 197 | [APP-196: Tier A review of the schema restore command and the scan-delta binding](0-backlog/APP-196.md) | [PH09](phases/PH09.md) | S15 | OPS | P1 | S | Backlog | APP-175 |
