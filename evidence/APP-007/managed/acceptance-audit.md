# APP-007 independent acceptance audit — draft

Recorded 2026-10-29T00:46:50.338322+00:00; repository HEAD `fe4ca0e0491e46e63c73c8358a75250bb8887f51`. **APP-007 acceptance is incomplete at this checkpoint.** No ticket status or implementation was changed. This audit reads retained receipts; the reviewer made no cloud calls and did not repeat cloud operations.

The managed runtime remains source `8f8c34ba0b5e47a55990b273fc6d555d41369791` / `sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22`. The newer committed source and uncommitted optional APP-006 callback configuration are separate identities. The disabled APP-007 foundation is not blocked merely because the optional callback has not deployed.

## Acceptance checklist

| Ticket item | Disposition | Evidence and remaining obligation |
|---|---|---|
| 1. Reproducible agreed managed environment, two-tenant access, migration, cleanup/rollback and actual identities | partial | Real managed foundation/store and recovery checks are substantiated. Tenant row/RLS authorization is APP-008; it is not fabricated here. Complete remaining negative behavior and final environment/configuration retention; reconcile fallback. Evidence: M1, M2, M3, M4, M5, M10, M11, L5. Remaining: G1, G2, G3, G5. |
| 2. Clean clone pinned install, initial migration and documented minimal configuration without committed secrets | substantially evidenced; final handoff reconciliation pending | Fresh CI checkout runs locked installs/build/image and exact PostgreSQL migration; real migrations and deployed readiness use runtime secrets, not sample passwords. Example is deliberately incomplete: substitute reviewed nonsecret outputs and use documented secret channel. This item does not require valid credentials in Git. Evidence: L2, L3, L4, L5, L7, L8, M1, M2, M7, S1, S8, S9, S10. Remaining: G5. |
| 3. One business binary, four constrained role interfaces, no privileged role selector | sufficient for foundation scope | Typed fixed interfaces reload distinct SQL identities; managed write-enabled DDL/DML/SET ROLE denial covers effective foundation grants. Shared-process compromise boundary is explicit. Signed internal tokens, complete grants and tenant/RLS remain later owning work. Evidence: L1, L3, M2, S3, S4, S5, S6, S7. Remaining: none within this foundation slice. |
| 4. Locked focused build/type/format/security CI; no premature app capability claims | sufficient at named committed revisions | SHA-pinned workflow actions, locked installs/builds and eight exact-source checks exist. fe4ca0e CI is not proof that uncommitted optional auth secret wiring is qualified or that its image is deployed. Full signed release/SBOM gate belongs APP-061. Evidence: L2, L4, L7, L8, S8, S10. Remaining: none within this foundation slice. |
| 5. Bounded trial costs/stop/cleanup, $100 limit, 2vCPU/8GiB, no LLM | substantially evidenced; final inventory/operating record pending | Retained reservation is $97.49 project total including $5 auth and $25 unallocated; actual stop-only Scheduler event and task convergence proved. It is an estimate/reservation, not final invoice or hard billing cap. Retained RDS/ALB/keys continue charging. Record final deployed sizing/identity and reconcile additional standalone work with reservation before a new window. Evidence: M8, M10, M11, P4, S2, S13. Remaining: G5. |
| 6. beta DNS/TLS public HTTPS, brief real /32 trial, private direct boundaries, inert routes and protected-work denial | partial | Actual local/OVH TLS200 and /32 external timeout plus subsequent public443 inventory are retained. Actual SG inventory demonstrates ALB-only3000/private5432 and no extra listener; no independent direct-port TCP probe claimed. Deployed8f route source has inert pages/health/assets only, but retained live public protected-route response evidence is missing. Evidence: M7, M9, L4, S7. Remaining: G4, G5. |

## Independent consistency checks

All 20 read-only consistency checks passed; exact assertions are in [the audit JSON](acceptance-audit.json). These validate the retained evidence, not an independently repeated managed run.

- Recomputed the six registry-bound source hashes from Git objects at `8f8c34b`.
- Verified two successful stopped migrations and the exact managed image; 88 explicit SQL checks and all 11 tenant-storage checks; exact two-version cleanup.
- Compared entire source/restored manifests independently and recomputed both canonical digests. Distinct DB resource identities and before-reseed claim are retained. Later cleanup closes the earlier restore receipt's historical `cleanup_pending` state.
- Matched credential-refresh task identity to actual scheduled-stop task; same task was ready beyond the helper's600-second maximum file lifetime, then stopped by the Scheduler role. This proves positive refresh, not expiry failure or customer-session policy.
- Checked all eight CI result records bind the exact `fe4ca0e` source; no claim that later working-tree edits passed that CI.

## Concrete remaining work

### G1: actual configuration fix pending

**Basis:** Reproducible pinned managed foundation (M12, M5, S13).

Source DB AutoMinorVersionUpgrade is recorded true. Reviewed explicit-false repair has exact-source CI but no actual update-complete/same-DB-identity receipt yet.

**Closure:** Retain completed guarded update and live false flag, unchanged source DB resource identity, engine16.13, image/stop/network and available healthy baseline; do not infer from template alone.

### G2: required negative managed behavior evidence absent

**Basis:** Environment plan explicitly requires negative missing-credential, wrong-region/key, unreachable-service behavior; local review also calls for credential expiry failure. (P3, L1, S3, S4, S6, S11, M3, M6).

Wrong tenant key/prefix denied in real managed trial; refresh beyond600s passes. Existing local expired-file/no-probe tests and source guards are useful, but missing-credential/wrong-region/unreachable-service and actual expiry-refusal managed-path receipts were not found.

**Closure:** Use isolated bounded negative invocations against the reviewed deployed foundation; retain expected refusal/sanitized error/no readiness or work success and task cleanup. No real credentials in evidence, no outage of shared services, no invalidating earlier positive results. If a case is rejected before networking by design, record actual invocation at that boundary instead of inventing a cloud denial.

### G3: rollback disposition incomplete

**Basis:** Ticket cleanup/rollback and chosen fallback; maintained review promised image rollout/rollback. (T1, L1, S2, M4, M5, M8, M11, D2).

Initial deployment, failed-attempt cleanup, foundation snapshot restore and service stop are observed. No prior task-definition/image rollback trial was located.

**Closure:** Retain a bounded actual chosen runtime rollback/recovery result, or explicitly reconcile initial-deployment fallback using observed stop/restored foundation and qualified baseline without claiming an unrun prior-image rollback. Full upgrade-downgrade-upgrade/canary waves remain APP-061/PH05, not added here.

### G4: public surface runtime receipt absent

**Basis:** Public foundation exposes only inert/identity surfaces; protected work denied. (T1, M7, S7, L4).

Live receipt proves ready200/TLS, not protected endpoint refusals or deployed HTML/security response. Deployed8f source has no business/identity routes and local image smoke proves CSP/no-store; no managed HTTP receipt currently binds these public-route results.

**Closure:** Retain a small exact-image actual-hostname HTTP check of allowed inert page and representative protected/query/command/import/auth/admin/reset paths and unsupported mutation methods returning denial, without exercising future unimplemented business flows. Direct-task-port TCP denial is additional confidence, not a separate invented mandatory test; current actual SG inventory already proves configured private boundary.

### G5: reviewable final operating record incomplete

**Basis:** Actual configuration identities, resource ownership/cleanup/cost record and truthful handoff. (T1, P3, P4, L1, S1, S2, M1, M3, M7, M8, M9, M10).

Retained records identify stack, DB, bucket, service/tasks, SGs, ALB/cert and image. Actual six role ARN/two key ARN/prefix map and deployed2vCPU/8GiB/UID/read-only settings were not retained in the audited public receipts. Current maintained docs still state no managed deployment/migration has run. Historical receipts retain legitimate then-pending flags.

**Closure:** Add sanitized observed configuration/role/key/sizing identities and final stopped/retained inventory with reservation/cleanup ownership. Update current-state narrative and ticket log, linking later proof while preserving earlier failures and historical pending receipts. Future image/config changes need fresh relevant checks and explicit new identities; optional APP006 secret wiring does not independently block APP007 disabled foundation.

## Design boundary retained

The controlling platform design D11/§6 requires one business binary with constrained role credentials; the demo D1/D2/D10/§10.7/§11.1 keeps managed RDS/S3/KMS from row one, reduces services to one2-vCPU/8-GiB task, substitutes typed in-process identity for signed service tokens, and uses pinned image/per-build qualification before the later full release path. ADR-003 expressly replaces permanent operator-IP access with public HTTPS and later application admission. The audit keeps those reductions and does not use them to waive tenant isolation or application access.

N9's two-app/no-code-change proof, full platform SR3/SR8, invite signup, admitted sessions, RLS, business queries/commands, application restore, audit chains and full phase journeys remain assigned work. This scaffold does not pass those requirements in full. PH01 still requires every ticket and its integrated gate; APP007 completion alone cannot authorize Phase2.

## Source index

Paths below are repository-relative; exact hashes are in the JSON.
- **T1:** `Tickets/2-in-progress/APP-007.md`
- **P1:** `docs/planning/QUALITY-GATES.md`
- **P2:** `docs/planning/SOURCE-GUIDE.md`
- **P3:** `docs/planning/SYNTHETIC-ENVIRONMENT.md`
- **P4:** `docs/planning/SYNTHETIC-ENVIRONMENT-COST.md`
- **P5:** `docs/architecture/ADR-003-PUBLIC-BETA-ACCESS.md`
- **D1:** `design/forma-generative-app-platform-v2/deliverables/v1/md/design.md`
- **D2:** `design/forma-northstar-demo/deliverables/v1/md/design.md`
- **D3:** `design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md`
- **D4:** `design/forma-generative-app-platform-v2/VALIDATION.md`
- **D5:** `design/forma-northstar-demo/SCENARIOS.md`
- **L1:** `evidence/APP-007/review.md`
- **L2:** `evidence/APP-007/local-preparation.json`
- **L3:** `evidence/APP-007/migration-repair/review.md`
- **L4:** `evidence/APP-007/migration-repair/image-qualification.json`
- **L5:** `evidence/APP-007/migration-repair/registry-image-receipt.json`
- **L6:** `evidence/APP-007/public-beta-infra/review.md`
- **L7:** `evidence/APP-007/scheduler-repair/ci.json`
- **L8:** `evidence/APP-006/hosted-callback/ci.json`
- **M1:** `evidence/APP-007/managed/managed-migrations.json`
- **M2:** `evidence/APP-007/managed/managed-sql.json`
- **M3:** `evidence/APP-007/managed/tenant-qualification.json`
- **M4:** `evidence/APP-007/managed/restore-comparison.json`
- **M5:** `evidence/APP-007/managed/restore-cleanup.json`
- **M6:** `evidence/APP-007/managed/credential-refresh.json`
- **M7:** `evidence/APP-007/managed/network-tls.json`
- **M8:** `evidence/APP-007/managed/scheduled-stop.json`
- **M9:** `evidence/APP-007/managed/beta-dns.json`
- **M10:** `evidence/APP-007/managed/retry-creation-request.json`
- **M11:** `evidence/APP-007/managed/initial-attempt-cleanup.json`
- **M12:** `evidence/APP-007/managed/auto-minor-upgrade-repair-review.json`
- **S1:** `app/README.md`
- **S2:** `infra/README.md`
- **S3:** `app/runtime/runtime.py`
- **S4:** `app/src/config.rs`
- **S5:** `app/src/roles.rs`
- **S6:** `app/src/readiness.rs`
- **S7:** `app/src/main.rs`
- **S8:** `.github/workflows/foundation.yml`
- **S9:** `app/.env.example`
- **S10:** `scripts/check-foundation.py`
- **S11:** `app/runtime/test_runtime.py`
- **S12:** `infra/scripts/foundation_restore_manifest.py`
- **S13:** `infra/src/stack.ts`
