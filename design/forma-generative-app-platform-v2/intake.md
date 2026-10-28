# Intake: forma-generative-app-platform-v2

Prepared October 22, 2026 for product review and factory setup. This working intake does not confirm a full design run. `input/` preserves the request and both definitions. [ALIGNMENT.md](ALIGNMENT.md) resolves differences; [VALIDATION.md](VALIDATION.md) supplies proposed numerical assumptions A1–A8.

## Level

- Level and why: L3, a platform of several services, multi-tenant, with its own stores, jobs and operations; confirmed by the head 2026-10-22 (K2).
- Page ask in the brief, if any: none; the L3 range (24 to 40 body pages) applies.
- Vendor claims (phase 7): verified. Reference cross-check (phase 7): run. Reader's edition (phase 8): produced, for the founder and a technical investor (`design-investor.md`). Reception (phase 9): produced; recipient: a technical YC-style early-stage investor reading the investor edition in one sitting of about an hour. Annotated edition (phase 10): produced, for a developer with two years' experience.

## Rigor mode

- Proposed mode: standard, confirmed by the head 2026-10-22 (K2); review at L3 strength; pause points become written summaries, the run continues, stopping only for the head's own decisions.
- Confirmed by the head on: 2026-10-22

## Document kind

- Kind: target
- Run by its users (library, framework, tool): not applicable, a hosted platform.
- Code claims (phase 7, as-built kinds, M27): not applicable to a target; pinned source evidence still backs every [built] claim.

## Rigor mode, kind and level (as prepared)

- **Mode:** rigorous proposed, matching GUIDE section 6's L3 expectation and the tenant, generation, data and mutation boundaries. `[assumed: A9; the head may amend the mode at design kickoff]`
- **Kind:** target. Released Forma 3.0 and prototypes are evidence inputs; the company platform has not been established as built. `[answered: investigation and request]`
- **Level:** L3 proposed: platform with app lifecycles, query and record stores, jobs, access and operations. `[assumed: A9]`
- **Page ask/deadline:** none supplied; use factory L3 budgets. No old MVP date transfers. `[assumed: A9]`
- **Confirmed by head:** not recorded for this new task. Phase 0 is prepared, not complete.
- **Run by users:** hosted managed browser product, not a library customers operate. Forma Stack remains a separate dependency.
- **Kept:** industry-agnostic platform, paired tasks, Northstar leading example, original inputs and historical deliveries.
- **Hard choices:** (1) typed app definition versus admitted markup; (2) source queries plus transactional records; (3) identity/publication policy; (4) canvas/runtime adaptation to 3.0; (5) model disclosure and admission.
- **Blind targets/alternatives:** optional target-design technique, proposed for representation/storage. Prior investigations are not blind independent designs. `[assumed: A9]`
- **Code claims:** full as-built audit is not this document kind; built/reuse assertions still need pinned source evidence. Normal mechanical, security, vendor-claim, xref and benchmark work remains.
- **Editions/reception:** reader and annotated editions plus reception remain planned, no inherited waivers. `[assumed: A10]`

| Dependency | Evidence |
|---|---|
| getforma-dev/forma | v3.0.0 → `2006423b21fe0f8dd5a510bd09bfe3b8748327a5`; source snapshot in review manifest |
| Historical platform | Retained deliverable v4, target; folder `forma-generative-app-platform` |
| Historical insurance MVP | Retained deliverable v5, target; folder `forma-insurance-demo` |
| Built-app donors | Pre-3.0 source snapshots, not qualified platform implementation; see SOURCES |

## Reader edition

Proposed audience: founder and technical investor; engineering record for implementation reviewers. Use company home → Northstar → second app → common platform → proof and risks. Requirements, architecture, flows, security and risks deserve short chapters; components/data/API can use summaries and pointers. Operations and alternatives explain support, cost and capability limits. Reception persona: a technical YC-style early-stage investor assessing product identity, working evidence and customer learning in one sitting. This is simulated reception, not actual YC feedback. Annotated reader: engineer with two years' experience. `[assumed: A10; change narrative/reception if recipient differs]`

## A. What it is and who asked

1. Forma lets companies create, run, adapt and share task-specific apps and saved work in a managed workspace. `[answered: user; primary PDF pp.1–4]`
2. Victor requested a step back from Northstar to the platform, with an MVP demonstration, October 22, 2026. `[answered: input/brief.md]`
3. Current deliverable is local factory preparation; later design is Markdown/PDF with normal editions. No page limit/deadline supplied. `[answered: request context; assumed: A9]`
4. Immediate reader Victor; later builders and technical investors. Judge generality, credible implementation path and safe persistent work first. `[assumed: A10]`
5. New product on released Forma 3.0, selected prototypes and reconciled historical designs. `[answered: earlier request/evidence]`

## B. Users

6. Enterprise team members author and use apps manually; future integrations are programmatic consumers. Browser first. `[answered: primary PDF pp.6,13–16]`
7. Builders edit; users run/update permitted records; reviewers publish per policy; admins manage membership/policy. Roles may overlap. Frequency uses A1/A2. `[answered: PDF pp.6,13; assumed: A1/A2]`
8. Operators need jobs, budgets, incidents, restore and deletion visibility with controlled access. Actual operator/team unknown. `[assumed: A8; open: P2 before hosted pilot]`

## C. Scale and shape

9. No 3/6/12-month forecast. Candidate 10-org/100-seat envelope is not a market prediction. `[assumed: A1]`
10. 30 active users → 3 interactions/s → up to 18 query operations/s; separately test 2 record submissions/s and a hot record. `[assumed: A2]`
11. No measured read/write ratio; test query-heavy and concurrent-form-edit workloads separately. `[assumed: A2]`
12. A3/A4 set candidate volumes/bounds; cost originals, normalized data, records, audit and backups separately. `[assumed: A3/A4]`
13. Six-view fan-out and a noisy organization are initial trials; bound per-org/global concurrency. `[assumed: A2/A5]`

## D. Non-functional targets

14. Candidate client p50/p99: navigation 250 ms/1 s, warm query 500 ms/2 s, record acknowledgement 300 ms/1 s. VALIDATION defines workload/cold-path reporting. `[assumed: A2/A5]`
15. No contractual availability number. Visible failure and model-independent published operation required; support terms open. `[assumed: A8; open: P2 before pilot]`
16. Publication, record commands/outcomes and version adoption require atomic decisions/CAS and read-your-write. Search may lag but reauthorizes; personal state saves expose failure. `[assumed: A11, C02/C06–C10]`
17. Normal-restart durability; candidate pilot RPO 24 h/RTO 4 h with restore evidence. `[assumed: A7]`
18. Generation/retrieval may degrade visibly. Authorization, record integrity and provenance never fail open. `[assumed: A11]`
19. Exact fixture arithmetic/record outcomes; uncertain model interpretation cites receipts. Approximate retrieval stays within authorized candidates. `[assumed: A11]`
20. Budget unknown. Meter components/model quotas and derive monthly cost; no prior budget transfers. `[assumed: A8; open: P2 before deployment]`
21. Reduce scale, automation/integrations and deployment breadth; retain two-app creation, forms and authority checks. `[answered: scope; assumed: A1–A8]`

## E. Where it lives

22. Single managed browser offering, candidate single region. Hosting, residency and model-processing location to be chosen. `[answered: PDF pp.13–16; assumed: A8/A12; open: P2 before live data]`
23. Forma 3.0 shell/renderers plus selected GenUI/Vector source. Identity adapter, query store and transactional records require explicit design choices. `[answered: ALIGNMENT; assumed: A11/A12]`
24. Team size/skills/support hours unknown; do not schedule from invented headcount. `[assumed: A8; open: P2 before delivery commitment]`

## F. Data

25. Company files, schemas, logs, financial rows and employee records can be sensitive. Synthetic demo first; classify actual pilot data. No app credential collection feature. `[answered: PDFs; assumed: A12]`
26. A6 is for synthetic qualification only. Real-data deletion, derived data and backup expiry require policy. `[assumed: A6/A12; open: P2 before live data]`
27. Forma owns app definitions/publications, internal records, saved work and audit. Connected sources stay authoritative externally; imported snapshots are immutable evidence. `[answered: primary PDF pp.11–12; assumed: A11]`

## G. Security

28. Consider cross-tenant users, insiders, injected evidence/model output, malicious files and compromised dependencies/operators. Full threat model in phase 4. `[assumed: A11]`
29. No certification/regime requested or claimed. Assess actual customer requirements before real-data pilot. `[answered: request; assumed: A12]`
30. No app, model, stale job or link may disclose information or mutate work beyond current caller authority. `[assumed: A11; C01/C06/C07/C11/C12]`
31. Provider controls do not replace app authorization, generation admission or record integrity. Map shared responsibilities when selected. `[assumed: A12]`

## H. Integrations

32. CSV/bounded XLSX and chosen generation provider first. Connector, MCP host and external action later; connected writes excluded initially. `[answered: primary PDF; assumed: A12]`
33. Import/generation may acknowledge queued work. Record commands retain outcomes for uncertain acknowledgements; timeout does not justify duplicate intent. `[assumed: A11; C07/C15]`

## I. Closing

34. Design start: mode/level, representation/storage and measured envelope. Before pilot: customer, providers, region, retention, operator/budget. None blocks preparation. `[open: P1 at design start, P2 before deployment; assumed: A1–A12]`
35. Failure: security-only product disguised as platform, hidden per-app coding, nonpersistent forms, unsafe sharing, incorrect evidence or no recurring demand. `[answered: goals; proposed validation]`
36. Earlier security-first prescription conflicts with current intent; primary app-platform definition wins. Release/prototypes do not establish a working managed platform. `[answered: ALIGNMENT]`

## Additional assumptions

- **A9:** rigorous/L3 target with normal budgets/gates; change process if amended, not evidence facts.
- **A10:** engineering record plus founder/investor reader and learner edition; change narrative/reception if audience differs.
- **A11:** typed definitions/trusted renderers, common queries/transactional records and organization policy. If rejected, redo shared contracts in both designs and cost adapters to old platform decisions.
- **A12:** synthetic demo first; choose real customer/identity/model/region/data policy before pilot. Earlier live data requires resolving those dependencies first.

## The head's decisions of 2026-10-22 (evening): the kickoff rulings

*Source: the head's message in this session, 2026-10-22, after reading `review/foundation-review.md`:
"Ok do all based on your recommendations. One thing fAuth needs to be confirmed its current build
state is viable for a demo. It doesn't have to be fully done as the demo doesn't care. But if it
passes the demo then yea it can be used. We have to agree and ask if it will hurt or wow in our demo."*

| Item | Ruling | Consequence |
|---|---|---|
| K1 Representation | **B**: a typed app definition rendered by first-party compiled components; design A v4's markup path (D13) becomes a later custom-widget tier | C04 is the first-release language; FORMA-237 to 242 are off the first-release path (their Log lines carry both readings); the H0 to H11 table in `ALIGNMENT.md` applies in its "Under B" column |
| K2 Mode and review | **standard** mode, review at L3 strength (two fresh reviewers, the security reviewer, the pairwise read against the L3 exemplar), level L3, kind target; the demo standard, L2, target | "Do all" is read as the forma-platform precedent: the lead writes a summary at the standard pause points after phases 1, 2, 4 and 6 and continues, stopping only for a decision that is the head's; the head may stop the run at any summary |
| K3 Stack tickets | FORMA-232 (raised to P1) and FORMA-246 are assumed done for the design | Each is cited where the design leans on it; neither counts as built until its acceptance tests pass |
| K4 Identity | **fAuth first, conditional**: its current build (`getforma-dev/gatewasm` `b750ca53`, the live `auth.getforma.dev`) must be confirmed viable for the demo, judged with the head on whether it hurts or wows; it need not be complete | The lead assesses it during phase 1 and stops at the phase-1 summary with a verdict; the identity adapter boundary (one verified-principal check, provider behind it) holds either way, so the design does not fork on the answer |
| K5 Stores | Immutable Parquet source revisions queried by DuckDB; PostgreSQL for records, saved work, publications and audit | D19 retained; the transactional record store is new; one data service owns both behind the receipt and command contracts |

Assumptions the rulings leave to the lead, recorded as the design's own: the hosting provider and
region follow design A (AWS, one region) unless the head says otherwise [assumed: A13]; the
generation provider is Claude through the provider interface design A named, re-verified at the
claims phase [assumed: A14]; no customer, date, team or budget is implied by any of this.

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

- Edition: `design-investor.md`, built and checked with `--edition investor`.
- Audience: the founder presenting, and a technical investor (a YC-style partner with an engineering background) reading alone.
- Shape: by the investor's questions, eight chapters: what it is and why now; how it works (two pictures and the eight-step scenario); why the second app is the proof; what keeps organizations apart; what a number means (receipts and the record command); what it costs and how one founder runs it; what we do not know yet and what to do first; the map to the record.
- Per section of the record: 1 a chapter in plain words; 2 a paragraph; 3 a paragraph; 4 a ranked list; 5 a chapter with diagrams 01 and 02; 6 a paragraph; 7 a paragraph on flow 3 with diagram 06; 8 and 9 pointers; 10 a brief chapter (the top threat rows, the accepted risks); 11 a brief chapter (cost, SLOs, rollout, one founder); 12 a paragraph (why not Retool, Airtable, Glide or artifacts); 13 a chapter; appendix a pointer.
- Length: half the L3 range, 12 to 20 pages; the summary a half page.
- Recipient (phase 9): a technical investor at a YC-style firm, receiving the edition's pages and the brief.
- Sitting: one hour, alone, before a partner meeting.
- Annotated edition (phase 10): for a developer with two years' experience who joins to build it.
