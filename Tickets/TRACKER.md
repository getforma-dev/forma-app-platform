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
| 6 | [APP-006: Revalidate fAuth contract and integration prerequisites](9-blocked/APP-006.md) | [PH01](phases/PH01.md) | S01 | IAM | P0 | M | Blocked | APP-001 |
| 7 | [APP-007: Scaffold reproducible browser/Rust workspace and one binary's role interfaces](4-done/APP-007.md) | [PH01](phases/PH01.md) | S02 | FND | P0 | L | Done | APP-001, APP-002 |
| 8 | [APP-008: Establish tenant-bearing schema, database roles and deny-by-default authorization](1-selected/APP-008.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Selected | APP-007 |
| 9 | [APP-073: Establish bounded invite and trial admission with owner-issued codes](0-backlog/APP-073.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-008 |
| 10 | [APP-009: Implement fAuth-backed session lifecycle](0-backlog/APP-009.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-006, APP-008, APP-073 |
| 11 | [APP-074: Qualify invite-only signup and public beta access](0-backlog/APP-074.md) | [PH01](phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-006, APP-009, APP-073 |
| 12 | [APP-010: Persist and admit a minimal typed definition and work binding](0-backlog/APP-010.md) | [PH01](phases/PH01.md) | S02 | FND | P0 | L | Backlog | APP-002, APP-008 |
| 13 | [APP-011: Package reproducible synthetic sources and expected outcomes](0-backlog/APP-011.md) | [PH01](phases/PH01.md) | S02 | VAL | P0 | M | Backlog | APP-002, APP-007 |
| 14 | [APP-012: Deliver a minimal authorized query with receipt](0-backlog/APP-012.md) | [PH01](phases/PH01.md) | S02 | QRY | P0 | L | Backlog | APP-003, APP-005, APP-009, APP-010, APP-011, APP-074 |
| 15 | [APP-013: Commit one declared internal record and recover outcome](0-backlog/APP-013.md) | [PH01](phases/PH01.md) | S02 | REC | P0 | L | Backlog | APP-012 |
| 16 | [APP-014: Prove command retries, conflicts and authorization race ordering](0-backlog/APP-014.md) | [PH01](phases/PH01.md) | S02 | REC | P0 | M | Backlog | APP-013 |
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
| 29 | [APP-027: Port measured canvas interaction and structured alternative](0-backlog/APP-027.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | L | Backlog | APP-003, APP-025 |
| 30 | [APP-028: Complete the trusted native renderer catalog](0-backlog/APP-028.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | L | Backlog | APP-027 |
| 31 | [APP-029: Verify accessible component states and equivalent focused workflows](0-backlog/APP-029.md) | [PH02](phases/PH02.md) | S05 | UX | P1 | M | Backlog | APP-028 |
| 32 | [APP-030: Generalize forms and commands without weakening transaction invariants](0-backlog/APP-030.md) | [PH02](phases/PH02.md) | S05 | REC | P0 | L | Backlog | APP-013, APP-014, APP-028, APP-029 |
| 33 | [APP-031: Separate saved work from personal presentation](0-backlog/APP-031.md) | [PH02](phases/PH02.md) | S05 | GOV | P1 | L | Backlog | APP-026, APP-030 |
| 34 | [APP-032: Complete AppDefinition schema and registry admission](0-backlog/APP-032.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-010, APP-022, APP-028, APP-029, APP-030 |
| 35 | [APP-033: Build direct composition/editor controls](0-backlog/APP-033.md) | [PH03](phases/PH03.md) | S06 | AUT | P1 | L | Backlog | APP-032 |
| 36 | [APP-034: Implement patches, semantic diff and undo revisions](0-backlog/APP-034.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-033 |
| 37 | [APP-035: Isolate preview execution and draft records](0-backlog/APP-035.md) | [PH03](phases/PH03.md) | S06 | AUT | P0 | L | Backlog | APP-023, APP-030, APP-034 |
| 38 | [APP-036: Prove authoring generality before AI polish](0-backlog/APP-036.md) | [PH03](phases/PH03.md) | S06 | VAL | P0 | M | Backlog | APP-033, APP-035 |
| 39 | [APP-037: Implement versioned publication policy and lifecycle rules](0-backlog/APP-037.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-008, APP-032, APP-034 |
| 40 | [APP-038: Commit exact publications atomically under current policy](0-backlog/APP-038.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-037 |
| 41 | [APP-039: Reviewer inbox over admitted bytes](0-backlog/APP-039.md) | [PH03](phases/PH03.md) | S07 | GOV | P1 | L | Backlog | APP-035, APP-037, APP-038 |
| 42 | [APP-040: Implement explicit work-version adoption and rollback](0-backlog/APP-040.md) | [PH03](phases/PH03.md) | S07 | GOV | P0 | L | Backlog | APP-031, APP-037, APP-038 |
| 43 | [APP-041: Ownership, app placement, retirement and missing dependencies](0-backlog/APP-041.md) | [PH03](phases/PH03.md) | S07 | GOV | P1 | L | Backlog | APP-026, APP-037, APP-038, APP-040 |
| 44 | [APP-042: Build private compatible catalog retrieval](0-backlog/APP-042.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-037, APP-038, APP-041 |
| 45 | [APP-043: Configure provider/disclosure adapter and budget control](0-backlog/APP-043.md) | [PH03](phases/PH03.md) | S08 | AI | P0 | L | Backlog | APP-015, APP-032 |
| 46 | [APP-044: Compose, adapt and propose changes as durable jobs](0-backlog/APP-044.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-034, APP-042, APP-043 |
| 47 | [APP-045: Enforce the five assistant tool boundaries](0-backlog/APP-045.md) | [PH03](phases/PH03.md) | S08 | AI | P0 | L | Backlog | APP-030, APP-044 |
| 48 | [APP-046: Explain authorized results from receipts](0-backlog/APP-046.md) | [PH03](phases/PH03.md) | S08 | AI | P1 | L | Backlog | APP-023, APP-043, APP-045 |
| 49 | [APP-047: Freeze and run the 15-intent evaluation](0-backlog/APP-047.md) | [PH03](phases/PH03.md) | S08 | VAL | P0 | M | Backlog | APP-044, APP-045, APP-046 |
| 50 | [APP-048: Separate app, work, source, record and export grants](0-backlog/APP-048.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-023, APP-031, APP-037, APP-038 |
| 51 | [APP-049: Protect source-dependent saved and derived information](0-backlog/APP-049.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-048 |
| 52 | [APP-050: Deliver and measure revocation end to end](0-backlog/APP-050.md) | [PH04](phases/PH04.md) | S09 | SHR | P0 | L | Backlog | APP-024, APP-040, APP-048, APP-049 |
| 53 | [APP-051: Export an authorized labeled snapshot](0-backlog/APP-051.md) | [PH04](phases/PH04.md) | S09 | SHR | P1 | L | Backlog | APP-015, APP-048, APP-049, APP-050 |
| 54 | [APP-052: Exercise handoff, independent reuse and all data surfaces](0-backlog/APP-052.md) | [PH04](phases/PH04.md) | S09 | VAL | P0 | M | Backlog | APP-041, APP-050, APP-051 |
| 55 | [APP-053: Prove synthetic restart, restore, deletion and quota recovery](0-backlog/APP-053.md) | [PH04](phases/PH04.md) | S09 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-015, APP-016, APP-024, APP-043, APP-048, APP-049 |
| 56 | [APP-054: Run the complete Northstar journey from imported evidence](0-backlog/APP-054.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-019, APP-025, APP-039, APP-047, APP-052, APP-053 |
| 57 | [APP-055: Create Spend Review through the same live authoring path](0-backlog/APP-055.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-054 |
| 58 | [APP-056: Run failure, security and accessibility release suite](0-backlog/APP-056.md) | [PH04](phases/PH04.md) | S10 | VAL | P0 | M | Backlog | APP-047, APP-052, APP-053, APP-055 |
| 59 | [APP-057: Package reproducible demo evidence and rehearsal](0-backlog/APP-057.md) | [PH04](phases/PH04.md) | S10 | VAL | P1 | M | Backlog | APP-056 |
| 60 | [APP-058: Record pilot scope and real-identity prerequisites](9-blocked/APP-058.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Blocked | APP-057 |
| 61 | [APP-059: Prepare immutable role-separated hosted infrastructure](0-backlog/APP-059.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-007, APP-008, APP-058 |
| 62 | [APP-060: Authenticate service calls with constrained role identities](0-backlog/APP-060.md) | [PH05](phases/PH05.md) | S11 | IAM | P0 | L | Backlog | APP-059 |
| 63 | [APP-061: Enforce supply-chain and reversible deployment gates](0-backlog/APP-061.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-059, APP-060 |
| 64 | [APP-062: Complete append-only audit chains and immutable anchoring](0-backlog/APP-062.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-037, APP-038, APP-053, APP-059, APP-060 |
| 65 | [APP-063: Control support access and emergency administrative operations](0-backlog/APP-063.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062 |
| 66 | [APP-064: Apply complete retention and source deletion policy](0-backlog/APP-064.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-016, APP-048, APP-049, APP-058, APP-059, APP-060 |
| 67 | [APP-065: Execute reversible deletion holds and complete organization erasure](0-backlog/APP-065.md) | [PH05](phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062, APP-063, APP-064 |
| 68 | [APP-066: Build admin and operator workflows](0-backlog/APP-066.md) | [PH05](phases/PH05.md) | S11 | OPS | P1 | L | Backlog | APP-041, APP-059, APP-060, APP-062, APP-063, APP-064, APP-065 |
| 69 | [APP-067: Instrument SLOs, cost and payload-safe telemetry](0-backlog/APP-067.md) | [PH06](phases/PH06.md) | S12 | OPS | P1 | L | Backlog | APP-059, APP-060, APP-062, APP-063 |
| 70 | [APP-068: Exercise restore and failure drills](0-backlog/APP-068.md) | [PH06](phases/PH06.md) | S12 | OPS | P0 | L | Backlog | APP-061, APP-064, APP-065, APP-067 |
| 71 | [APP-069: Qualify load, noisy neighbor and budgets at declared limits](0-backlog/APP-069.md) | [PH06](phases/PH06.md) | S12 | VAL | P0 | M | Backlog | APP-024, APP-059, APP-060, APP-067 |
| 72 | [APP-070: Independent release security and accessibility review](0-backlog/APP-070.md) | [PH06](phases/PH06.md) | S12 | VAL | P0 | M | Backlog | APP-062, APP-063, APP-064, APP-065, APP-068, APP-069 |
| 73 | [APP-071: Publish operator/run/release/rollback and incident runbooks](0-backlog/APP-071.md) | [PH06](phases/PH06.md) | S12 | OPS | P0 | M | Backlog | APP-066, APP-067, APP-068, APP-070 |
| 74 | [APP-072: Evaluate one real recurring workflow and second-person reuse](0-backlog/APP-072.md) | [PH06](phases/PH06.md) | S12 | VAL | P1 | M | Backlog | APP-058, APP-070, APP-071 |
