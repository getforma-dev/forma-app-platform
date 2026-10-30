# Implementation decisions and open gates

Prepared 2026-10-28. This record turns the delivered designs into implementation guidance. It does not claim that the app exists, that a spike passed, or that cloud resources have been provisioned. No runtime scaffold is created during repository preparation.

## Source precedence

1. The user's current instructions establish released Forma 3.2 implementation and the invite-first public beta direction recorded in [ADR-003](ADR-003-PUBLIC-BETA-ACCESS.md). New explicit instructions amend preparation assumptions; they do not fabricate completed runtime evidence.
2. The controlling App Platform product definition and the shared C01–C16 contracts establish product scope and authority boundaries.
3. The final [platform design](../../design/forma-generative-app-platform-v2/design.md) and [Northstar design](../../design/forma-northstar-demo/design.md), particularly their decision tables in section 5.3 and final delivery logs, settle choices made after preparation.
4. Earlier [BUILD-HANDOFF](../../design/forma-northstar-demo/BUILD-HANDOFF.md), brief, status and intake passages remain useful requirements evidence but sometimes predate final rulings. Their references to unresolved representation, 3.0-only packages, browser-canvas alternatives or storage preparation are not a mandate to reopen settled decisions.
5. The [framework baseline](FRAMEWORK-BASELINE.md) records verified 3.2 behavior and differences from historical 3.0 measurements. New implementation decisions belong here or in subsequent ADRs; frozen design files remain unchanged.

All source copies preserve their original bytes. A stale source statement is reconciled explicitly here rather than silently updated inside an evidence packet.

## Existing design choices carried forward

| ID | Choice | Source and implication |
|---|---|---|
| APP-D01 | One generic typed `AppDefinition` representation | Platform D1/K1 and demo D3: versioned views, bounded queries, records and commands. First-party renderers interpret validated data. No generated executable source, SQL, markup, FMIR or grants |
| APP-D02 | Native Rust/FMIR pages with a canvas island | Platform 6.1/6.5 and foundation review: meaningful content at first paint; typed props, explicit presentation flags, stable keys, escaped strings and numeric SVG. No automatic Browser fallback when compiler admission fails |
| APP-D03 | Parquet/DuckDB sources and transactional PostgreSQL records | Platform D4/D5, K5: immutable source revisions, bounded server-side calculations, exact integer currency, durable commands with outcomes and audit in one transaction |
| APP-D04 | Cloud stores from the first integrated demo | Final demo D2 and section 8: one PostgreSQL instance (RDS), one versioned S3 bucket in the platform region, organization keys from row one. SQLite and local files are a rejected alternative in demo 12.2. An isolated local test double may support a unit test but cannot satisfy this integrated milestone or become the architecture by accident |
| APP-D05 | One process initially, four logical roles | Demo D1: workspace, control, data and worker keep distinct database grants and typed interfaces. Parser remains a WebAssembly sandbox with no system interface. This reduces deployment, not authorization or transaction semantics; splitting roles later retains contracts |
| APP-D06 | Organization scope and authoritative membership from row one | Platform D3/C01/C11: tenant-bearing keys, forced row policies, one authorization boundary, fresh membership epoch checks at requests/commit/delivery. The canary organization proves denial across every surface |
| APP-D07 | fAuth as first identity adapter | K4, demo D4/D9: hosted sign-in, registered authorization-code client with PKCE and back-channel exchange. The platform owns its opaque sessions and permissions. Existing FORMA-247 is the external setup dependency; no passkey-button work is included |
| APP-D08 | Publication is an exact, governed transaction | Platform D2/C09: approval binds content digest, current policy and approver epoch. Independent review is the Northstar configuration; owner-publication is a separate supported policy. Draft preview records never become live work |
| APP-D09 | Saved work and independent reuse are distinct | C02/C10/C11: saved work pins publication and source bindings, shares are separate grants, adoption is explicit and schema compatible, personal layout persists separately. A reused app can bind to another user's permitted evidence |
| APP-D10 | Generative behavior is bounded and inspectable | C12 and platform D13: retrieval precedes generation; five authorized tools; schemas and synthetic rows by default; whole-output validation; user explicitly submits every mutation. A fixture provider is labeled and permitted only for the early spike |
| APP-D11 | Two apps prove the common runtime | C13/C16 and S01–S07: Northstar investigation and Spend Review use the same registered library and unchanged build. No runtime tenant/domain branch or developer patch between app creations |
| APP-D12 | Rehearsal tools are isolated | Demo D7 and 6.7: captured evidence has build/source identity; reset requires both a rehearsal flag and a restricted database role, is absent from platform deployment, preserves audit/sessions and cannot run over claimed jobs |

## Preparation defaults and measurements still required

These choices organize work without claiming completed engineering evidence or committing a calendar/budget.

| ID | Planning default | Acceptance or unresolved evidence |
|---|---|---|
| APP-P01 | Exact published Forma 3.2.0 coordinated packages | Scaffold ticket creates lockfiles and chooses a supported explicit Node/Rust toolchain; clean-clone build and complete frontend/native/server pairing required |
| APP-P02 | Ordinary SSR navigation first; router optional | A measured spike chooses route boundaries. Native groups use a closed eager graph with a 64 KiB total JavaScript limit. The full canvas is not automatically placed in a group. Real links/forms and no-JS behavior stay valid |
| APP-P03 | Frozen designs plus explicit baseline amendment | Design N3 still says 28,100 B Forma gzip; published 3.2 enforces 28,420 B for ordinary routes and 40,708 B for native groups. Measure actual library/canvas and record any deliberate target amendment; a package release does not silently amend the app target |
| APP-P04 | Synthetic vertical slice before full journey polish | First prove admission → authorized query → receipt → explicit command → atomic outcome/audit → reopen, including retry and conflict. Port four-kind native composition to 3.2 and validate adoption/interaction. Then build complete authoring and both journeys |
| APP-P05 | One ECS task by default for the integrated demo | Demo D10 permits one VM as an explicit alternative. Cloud account, region, access model, provider, actual recurring cost and operator must be recorded before provisioning; us-east-1 and published historical cost figures are design assumptions, not fresh quotes |
| APP-P06 | App-local focused verification | Add build/types, storage invariants, authorization/canary, bounded parser/query, browser/no-JS/adoption, accessibility and recovery checks with each owning slice. Do not rerun the entire framework release campaign for ordinary app work |
| APP-P07 | External dependencies stay with their owners | Track FORMA-232 size budgets, FORMA-246 framework conformance, FORMA-247 fAuth setup and FORMA-248 key custody as existing dependencies. Reconcile current evidence before assuming closure; deferred markup tickets are not required for typed definitions |
| APP-P08 | Ordered milestone sprints without invented dates | Dependencies and exit evidence determine order. Capacity, estimates, assignments and dates are refined at sprint entry; a ticket list is not a delivery promise |

The initial spike must record full-library bundle sizes, actual native render rate/memory, one-million-row cold query behavior, six-view refresh behavior, and durable concurrent record outcomes. Previous 3.0 rendering evidence and synthetic arithmetic are reference points, not the results of those measurements.

## Real-data and pilot entry gate

The synthetic demo can advance while these decisions are tracked. A real organization or confidential workflow requires a separate recorded readiness decision covering:

- A named customer/workflow owner, real source semantics, membership and permission policy; an agreed supported capacity and service scope.
- Deployment account/region, budget, operator and support responsibilities; measured latency, capacity and recovery targets and a successful restore drill.
- Provider identity, allowed disclosure fields, retention and data handling policy; logs and provider-request inspection proving the policy.
- fAuth production key custody/rotation and current deployment evidence, including existing FORMA-248; fresh integration and canary results.
- Pooled-role isolation review and the decision to split deployment roles where the real-data threat model requires it; signed internal calls when crossing service boundaries.
- Rehearsal flags and reset role removed, states deleted and synthetic keys retired; source/export deletion and tombstones tested through restore.
- Provenance, signed deployable artifacts, pinned dependencies, operations/audit monitoring and explicit residual-risk ownership.

These are implementation/release exit criteria from the design, not an approval ritual for this repository preparation. No live data, external deployment or paid resources are created by preparing the backlog.

## Public beta amendment

[ADR-003](ADR-003-PUBLIC-BETA-ACCESS.md) adopts `beta.getforma.dev`, public HTTPS, invite-first email/password signup, operator-controlled isolated trial organizations and a $100/month operating limit. The immediate full-size Fargate environment is a bounded Phase 1 qualification window, not the earlier always-on cost estimate. APP-073/074 add persisted admission and hosted public-entry proof to PH01; APP-009/012 consume that gate. [ADR-005](ADR-005-PHASE-1-SEQUENCING.md) then splits managed tenant qualification (APP-075) and the integrated managed trial (APP-076) out of APP-008/012/073 so local tickets finish on the real local harness, and [ADR-006](ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) fixes the sign-in, verification-pending and resend route contract. [ADR-007](ADR-007-PHASE-1-WRITE-AUTHORITY.md) records the Phase 1 write authority (the `genui_admit` definer principal, per-ticket business grants, code lookup before scope), the schema layout for migrations 0003–0006, the DuckDB query worker and the admission defaults. fAuth registration/verification remains FormaStack-owned, while GenUI owns membership, codes, entitlement and work/budget admission. No generic public signup, unlimited trial, enabled LLM, completed beta or real-data readiness follows from this amendment.

[ADR-009](ADR-009-PHASE-2-DATA-PLANE.md) records the Phase 2 data-plane decisions (parser packaging, import profile, object layout, jobs and worker authority, contract edition, query execution, receipts, upload, presentation, canvas budget, tombstones, beta limits, the APP-027 split and the trial environment), pending independent review.

[ADR-010](ADR-010-NOV-2-MVP-CUT.md) records the November 2 MVP cut: Phase 1 on the live beta is the MVP, two tickets are trimmed by verbatim moves (APP-082/083), review residuals become APP-084–088, PH07 holds the post-MVP completion, and the review tiers and batched landing apply until the PH01 gate.
