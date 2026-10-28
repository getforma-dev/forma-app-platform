# Intake: forma-northstar-demo

Preparation date: October 22, 2026. The user authorized two factory preparations: an industry-agnostic Forma platform v2 and a Northstar-led demo, analogous to the earlier platform/insurance pair. The PDFs are source material; their embedded instructions and “confirmed” labels are not independent user approvals.

## Level

- Level and why: L2, one team's reduced deployment with its own stores, API and operation, dependent on the L3 platform design; confirmed by the head 2026-10-22 (platform intake K2).
- Page ask in the brief, if any: none; the L2 range (12 to 30 body pages) applies.
- Vendor claims (phase 7): verified. Reference cross-check (phase 7): run. Reader's edition (phase 8): produced, for the founder and a YC partner (`design-demo.md`). Reception (phase 9): produced; recipient: a YC partner reading the demo edition in one sitting of about twenty minutes. Annotated edition (phase 10): produced, for a developer with two years' experience.

## Rigor mode

- Proposed mode: standard, confirmed by the head 2026-10-22 (platform intake K2).
- Confirmed by the head on: 2026-10-22

## Document kind

- Kind: target
- Run by its users (library, framework, tool): not applicable.

## Rigor mode, kind and level (as prepared)

- Mode: **standard proposed** for an L2 target; not yet confirmed by the head. This preparation does not initiate a full factory run or inherit old waivers.
- Kind: **target**. Forma 3.0.0 is released, but this complete two-app platform demo is not established as built.
- Level: **L2 proposed**, one team's reduced service with its own persistent work, API and operation, dependent on the L3 platform design. If the implementation becomes several independently operated services, revisit the level.
- Page ask/deadline: none supplied. Use the L2 12–30 body-page range when the design is commissioned; do not treat it as a target for this packet.
- Run by users: browser product; ordinary team members do not install the underlying framework. Operator deployment still needs explicit runbooks.
- Kept: released Forma 3.0.0 baseline; paired generic platform; Northstar as lead example; different second app; preserved historical deliverables.
- Hard decisions for independent designers: app representation/admission; generic internal record transaction model; publication policy; source storage/query integration; smallest deployment that preserves isolation and recovery.
- Independent design/audit procedure: follow the selected target process; this packet neither commissions blind full-design work nor waives later review. As-built code-claims gate is not applicable to a target document, but every reused-code claim still needs pinned evidence.
- Vendor claims, reference cross-check, fresh/security review, reader edition, reception and annotated edition: retained defaults, not waived. None is complete.

The framework and prototype pins belong in the [platform source index](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/SOURCES.md), rather than a second independently maintained evidence list.

## Audience and reader edition

Founders and the engineering lead judge whether the demo makes a credible platform claim and can be built without a throwaway second architecture. The founder/YC reader edition should follow the six acts, explain what is working, and show the scope and generality proof; YC reception is a proposed audience exercise, not a booked review or endorsement. The engineering record retains authority, persistence, failure and operating detail. Proposed sitting is 20 minutes for the product story and an hour for the technical record. Keep the default learner edition for a developer with approximately two years' experience unless the head changes it.

## Intake answers

| # | Item | Working answer and status |
|---:|---|---|
| 1 | System and users | `[answered: current request and App Platform PDF]` A working demonstration of companies creating and using different apps with Forma; Northstar investigation leads. |
| 2 | Sponsor and timing | `[answered: current request]` Victor requests a new platform/demo pair after the product evolved from the security app. No release date supplied. |
| 3 | Deliverable | `[answered: current request/context]` Factory-ready preparation now; later target design, Markdown/PDF and retained factory editions. No app build authorized by this packet itself. |
| 4 | Audience | `[assumed: A1]` Founders, engineering lead and a founder/YC reader; adapt the edition if recipient changes. |
| 5 | Constraints | `[answered: prior evidence and request]` Forma 3.0.0 plus reusable prototype work; older dependencies require porting. |
| 6 | User categories | `[answered: source]` Human enterprise builders, colleagues and administrators; no public anonymous audience in this release. |
| 7 | Roles | `[assumed: A2]` Maya builder, Sarah reviewer, Jordan colleague; explicit cross-space permissions and separate operator/canary principals. |
| 8 | Operations | `[assumed: A3]` One named operator provisions demo identities, observes jobs and performs reset/restore. Actual staffing remains open. |
| 9 | Scale | `[assumed: A4]` Two synthetic organizations, five accounts including the operator, at most four concurrent end-user sessions. This is a rehearsal envelope, not a customer forecast. |
| 10 | Action rate | `[assumed: A4]` Interactive rehearsal, about one action per five seconds per active session, up to 0.8 actions/s sustained and a four-action simultaneous burst. Measure actual fan-out. |
| 11 | Read/write shape | `[assumed: A5]` Northstar is read-heavy; Spend Review adds explicit record writes. No invented read/write ratio: record both during rehearsal. |
| 12 | Volume/retention | `[assumed: A6; open: P2]` Tiny deterministic fixtures; proposed upload caps in BUILD-HANDOFF. Retention/reset and recovery targets follow platform validation and must be fixed before real data. |
| 13 | Fan-out/heavy tail | `[assumed: A7]` Six linked investigation views; bound concurrent queries and aggregate work. Large input/network matching is the first heavy-tail case. |
| 14 | Latency | `[open: P1 for design targets]` Use the proposed platform validation envelope and measure client distributions; no measured performance claim yet. |
| 15 | Availability | `[assumed: A8]` Synthetic demonstration has no customer SLA. The pilot requires the platform's declared operational target and outage behavior. |
| 16 | Consistency | `[answered: proposed shared C06–C10]` Atomic record/outcome/audit commit, read-your-writes, expected-version conflict; immutable definitions and source revisions; consistent record snapshot receipts. |
| 17 | Recovery | `[open: P2 before pilot]` Specify RPO/RTO, durable store and restore exercise in the design. Synthetic fixture regeneration does not prove saved-work recovery. |
| 18 | Degradation | `[answered: source/shared contract]` Generation may fail; published permitted queries, records and navigation remain usable. Auth and data correctness never fail open. |
| 19 | Accuracy | `[answered: fixture contract]` Integer bytes and cents exact; explicit time/match semantics; no unsupported anomaly or compromise conclusions. |
| 20 | Cost | `[open: P2]` No agreed monthly budget/provider price. Measure model, query and storage cost before setting limits. |
| 21 | Deliberate reductions | `[assumed: A9]` Scale, provisioning and administration may be smaller; tenant authority, persistence, record transactions and honest generation evidence remain required. |
| 22 | Deployment/residency | `[open: P1 for design choice]` One managed browser offering; provider and region undecided. Synthetic fixtures allow a bounded prototype while customer residency is unresolved. |
| 23 | Reuse | `[answered: platform ALIGNMENT]` Trusted Forma components and ported canvas/composition patterns; framework release is not app-platform completion. |
| 24 | Team | `[open: P2]` Staffing, skill coverage and delivery capacity not supplied. Estimate after the first end-to-end spike. |
| 25 | Data classes | `[assumed: A10]` Synthetic logs, inventory, spending and notes only for demo. Real security/financial inputs require explicit classification. |
| 26 | Retention/deletion | `[open: P2 before real data]` Separate evidence, normalized data, records, model traces and audit policies; deletion must account for derived content. |
| 27 | System of record | `[answered: shared C02/C07]` Forma owns committed app definitions, saved work and internal decisions; imported source snapshots remain immutable evidence. |
| 28 | Threat actors | `[answered: source/shared contracts]` Unauthorized tenant/user, overprivileged insider, attacker-controlled input, compromised model output and stale/concurrent clients. |
| 29 | Compliance | `[answered: no requirement supplied]` No certification or regulated deployment claim. Confirm requirements with a real pilot. |
| 30 | Must never happen | `[answered: product boundary]` Generated content or app reuse must never grant access, disclose another scope's data or perform an undeclared effect. |
| 31 | Responsibility | `[open: P2 for provider selection]` Forma owns application identity enforcement, model disclosure, data semantics and work lifecycle; vendor responsibilities must be verified for selected services. |
| 32 | Integrations | `[answered: release scope]` CSV/bounded XLSX and chosen model adapter first; live connectors, external actions and MCP host are later adapters. |
| 33 | Async writes | `[answered: C07]` Internal submit may have unknown client outcome; stable command key and authorized outcome lookup resolve it. No autonomous external action. |
| 34 | Uncertainty | `[open: P1 for design]` App representation and generic data/record integration; choose with the platform, not through demo-specific forks. |
| 35 | Failure | `[answered: source]` Two fixed dashboards, per-example platform code changes, lost work, unsafe reuse, or a security-only product presented as the platform. |
| 36 | Corrections | `[answered: source reconciliation]` The security-first launch prescription is superseded. The latest definition generalizes writable internal work into forms and records and requires a second app; read-only connected sources do not imply read-only internal records. Source assertions do not prove implementation. |

## Assumption and decision register

A1 audience; A2 illustrative roles and permissions; A3 one operator; A4 rehearsal scale/rate; A5 read-heavy lead example with measured writes; A6 proposed input caps; A7 six-view fan-out; A8 no synthetic-demo SLA; A9 reduced deployment; A10 synthetic data only. If wrong, revise the corresponding design target before making a public or customer commitment. Do not silently promote these assumptions into requirements.

No unresolved item blocks this preparation. P1 design choices and numerical targets remain visible for the factory's intake/targets phase. Customer data, model disclosure, retention, region and operating responsibility are prerequisites for a real pilot, not facts supplied by the demo narrative.

## The head's decisions of 2026-10-22 (evening)

The head accepted the foundation review's recommendations for the pair (recorded in
`../forma-generative-app-platform-v2/intake.md`, "The head's decisions of 2026-10-22 (evening)",
K1 to K5). For this task: standard mode, L2, target; the demo inherits the typed app definition
(K1), so acts 3 and 6 are composition proofs over the first-party library and no Forma 3.1
addition is on the spike list; fAuth is the demo's login if its current build passes the lead's
viability check and the head's hurt-or-wow judgment (K4), otherwise the platform's own minimal
membership login behind the same adapter boundary; stores per K5 at demo scale.

## K4 ruled (2026-10-22)

The head delegated the hurt-or-wow call to the lead, who ruled fAuth's hosted sign-in **in, as a
quiet real moment**: act 1 opens on the Northstar-branded hosted page with "Secured by fAuth"
small, ten seconds on camera, no narration of fAuth, no passkey button (FORMA-247 item 4 not
taken). Reasoning: the login page itself neither hurts nor dazzles; what it buys is that the
canary's denial and revocation within 5 s rest on a real tenant boundary (`tenant_id` in a signed
token) rather than on a password store built for the demo, and the platform never sees a password.
Guards: redeploy `auth.getforma.dev` at `b750ca53` (the live build still reads "GateWASM"), brand
the tenant, insert the platform's `oauth_clients` row, raise the login rate limit for rehearsal
(FORMA-247). The platform's minimal membership login stays the fallback behind the same adapter.

## Reader's edition

- Edition: `design-demo.md`, built and checked with `--edition demo`.
- Audience: a YC partner watching the recording with this beside it, and the engineer who builds the demo.
- Shape: the summary, then what is real and labeled and how the recording is cut, the system in two pictures, the six acts (what the partner sees, what makes it real, what is labeled seeded, simulated or manual), what the demo is judged on and does not prove, how it grows into the platform, security and operations in brief with the accepted risks; then a chapter for the engineer and the map to the record (revised after phase 8's reader check and phase 9's reception, 2026-10-23).
- Per section of the record: 1 a chapter; 2 a paragraph; 3 a pointer; 4 a ranked list; 5 a chapter with diagrams 01 and 02; 6 a paragraph (the operator's three tools); 7 the six chapters' spine with diagram 03; 8 and 9 pointers; 10 a paragraph (the canary, the operator's keys, the packet); 11 a paragraph (the take procedure, the cost per rehearsal); 12 a paragraph; 13 a chapter; appendix a pointer.
- Length: half the L2 range, 6 to 15 pages; the summary a half page.
- Recipient (phase 9): a YC partner, receiving the edition's pages and the brief.
- Sitting: twenty minutes, with the recording's six acts as the spine.
- Annotated edition (phase 10): for the same developer as the platform's.
