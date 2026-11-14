# OPS: Operations and security

Jobs, release controls, recovery, retention, support and pilot operations.

| Order | Ticket | Phase | Sprint | Category | Priority | Size | Status | Depends on |
|---|---|---|---|---|---|---|---|---|
| 17 | [APP-015: Implement durable jobs, leases and cancellation](../0-backlog/APP-015.md) | [PH02](../phases/PH02.md) | S03 | OPS | P0 | L | Backlog | APP-008, APP-009, APP-073 |
| 56 | [APP-053: Prove synthetic restart, restore, deletion and quota recovery](../0-backlog/APP-053.md) | [PH04](../phases/PH04.md) | S09 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-015, APP-016, APP-024, APP-043, APP-048, APP-049 |
| 61 | [APP-058: Record pilot scope and real-identity prerequisites](../9-blocked/APP-058.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Blocked | APP-057 |
| 62 | [APP-059: Prepare immutable role-separated hosted infrastructure](../0-backlog/APP-059.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-007, APP-008, APP-058 |
| 64 | [APP-061: Enforce supply-chain and reversible deployment gates](../0-backlog/APP-061.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-059, APP-060 |
| 65 | [APP-062: Complete append-only audit chains and immutable anchoring](../0-backlog/APP-062.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-013, APP-014, APP-037, APP-038, APP-053, APP-059, APP-060 |
| 66 | [APP-063: Control support access and emergency administrative operations](../0-backlog/APP-063.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062 |
| 67 | [APP-064: Apply complete retention and source deletion policy](../0-backlog/APP-064.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-016, APP-048, APP-049, APP-058, APP-059, APP-060 |
| 68 | [APP-065: Execute reversible deletion holds and complete organization erasure](../0-backlog/APP-065.md) | [PH05](../phases/PH05.md) | S11 | OPS | P0 | L | Backlog | APP-062, APP-063, APP-064 |
| 69 | [APP-066: Build admin and operator workflows](../0-backlog/APP-066.md) | [PH05](../phases/PH05.md) | S11 | OPS | P1 | L | Backlog | APP-041, APP-059, APP-060, APP-062, APP-063, APP-064, APP-065 |
| 70 | [APP-067: Instrument SLOs, cost and payload-safe telemetry](../0-backlog/APP-067.md) | [PH06](../phases/PH06.md) | S12 | OPS | P1 | L | Backlog | APP-059, APP-060, APP-062, APP-063 |
| 71 | [APP-068: Exercise restore and failure drills](../0-backlog/APP-068.md) | [PH06](../phases/PH06.md) | S12 | OPS | P0 | L | Backlog | APP-061, APP-064, APP-065, APP-067 |
| 74 | [APP-071: Publish operator/run/release/rollback and incident runbooks](../0-backlog/APP-071.md) | [PH06](../phases/PH06.md) | S12 | OPS | P0 | M | Backlog | APP-066, APP-067, APP-068, APP-070 |
| 81 | [APP-080: Qualify tenant races, drift and recovery on managed RDS](../0-backlog/APP-080.md) | [PH05](../phases/PH05.md) | S11 | OPS | P1 | L | Backlog | APP-077, APP-078 |
| 86 | [APP-086: Close the managed qualification and native campaign residuals (APP-075 and APP-008 reviews)](../0-backlog/APP-086.md) | [PH07](../phases/PH07.md) | S13 | OPS | P2 | M | Backlog | APP-075, APP-078 |
| 89 | [APP-089: Harden the container images: base CVEs and effective scan-on-push](../0-backlog/APP-089.md) | [PH02](../phases/PH02.md) | S03 | OPS | P1 | M | Backlog | APP-075, APP-078 |
| 125 | [APP-125: Developer workflow step 1: pins, dev profile, per-checkout venvs, no committed symlink](../4-done/APP-125.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | None |
| 126 | [APP-126: Developer workflow step 2: the front door (justfile, campaign catalog, VS Code workspace)](../4-done/APP-126.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125 |
| 127 | [APP-127: Developer workflow step 3: the beta deploy in Git (ops/beta, $PRIV per checkout, --approved gate)](../4-done/APP-127.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | M | Done | APP-126 |
| 128 | [APP-128: Developer workflow step 4: the genui-dev binary under process-compose (Compose PostgreSQL, in-process personas, clean profile)](../4-done/APP-128.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | M | Done | APP-125, APP-126 |
| 130 | [APP-130: Repository and delivery step C/D: one root Cargo workspace, the builder-cache sweep](../4-done/APP-130.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126, APP-127, APP-128 |
| 131 | [APP-131: Developer workflow step 5: CI green or quarantined (Foundation, Tenant foundation, the APP-013 campaign)](../4-done/APP-131.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126 |
| 133 | [APP-133: Repository and delivery: the release record (release.json, RELEASES.md, beta tags) and the module map](../4-done/APP-133.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-127, APP-130 |
| 136 | [APP-136: Developer guide from setup through beta delivery](../4-done/APP-136.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-125, APP-126, APP-127, APP-128, APP-130, APP-131, APP-133 |
| 137 | [APP-137: One private beta ledger across repository worktrees](../4-done/APP-137.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | S | Done | APP-127 |
| 161 | [APP-161: Spike — measure idle app costs and decide safe sleep/wake architecture](../0-backlog/APP-161.md) | [PH09](../phases/PH09.md) | S15 | OPS | P2 | M | Backlog | APP-140, APP-154 |
| 167 | [APP-167: Promote merged beta releases through GitHub Actions](../4-done/APP-167.md) | [PH08](../phases/PH08.md) | S14 | OPS | P0 | L | Done | None |
| 172 | [APP-172: Upgrade GenUI and GateWasm PostgreSQL from 16.13 to 16.15 after the image release](../4-done/APP-172.md) | [PH08](../phases/PH08.md) | S14 | OPS | P1 | L | Done | APP-167 |
| 173 | [APP-173: Review the November 5 base-image scan findings so beta promotion can continue](../4-done/APP-173.md) | [PH08](../phases/PH08.md) | S14 | OPS | P0 | S | Done | APP-167 |
| 175 | [APP-175: A reviewed operator schema release for migrations 21 and 22](../0-backlog/APP-175.md) | [PH09](../phases/PH09.md) | S15 | OPS | P1 | M | Backlog | APP-084, APP-169, APP-167 |
| 181 | [APP-181: Record the real live test charter and the beta notice](../0-backlog/APP-181.md) | [PH09](../phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-139 |
| 185 | [APP-185: Operator back office for admissions after the cutover](../0-backlog/APP-185.md) | [PH09](../phases/PH09.md) | S15 | OPS | P0 | M | Backlog | APP-181 |
| 186 | [APP-186: Renew the beta and the budget for the test window](../0-backlog/APP-186.md) | [PH09](../phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-181 |
| 187 | [APP-187: Email alarms and a budget alert](../0-backlog/APP-187.md) | [PH09](../phases/PH09.md) | S15 | OPS | P1 | S | Backlog | APP-181 |
| 188 | [APP-188: Operations runbook, restore drill and end-of-test purge](../0-backlog/APP-188.md) | [PH09](../phases/PH09.md) | S15 | OPS | P1 | S | Backlog | APP-185 |
| 194 | [APP-194: Schema release B: migration 0023](../0-backlog/APP-194.md) | [PH09](../phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-175, APP-182, APP-183, APP-186, APP-190, APP-191, APP-192 |
| 195 | [APP-189: Go-live: build day, rehearsal, codes and the class](../0-backlog/APP-189.md) | [PH09](../phases/PH09.md) | S15 | OPS | P0 | S | Backlog | APP-182, APP-183, APP-184, APP-185, APP-186, APP-187, APP-188, APP-190, APP-191, APP-192, APP-194 |
