# Requirements and evidence plan

This is a proposed acceptance plan for the factory. No row below claims an implementation or a passed runtime test. Source requirements come from the controlling app-platform PDF; architecture and operating numbers are recommendations. The demo's worked fixtures live in [SCENARIOS.md](../forma-northstar-demo/SCENARIOS.md).

## Product requirements

| ID | Required outcome | Evidence needed | Contracts |
|---|---|---|---|
| P01 | Forma is an organization app platform; spaces and domain labels are configurable | Two spaces and two meaningfully different apps on one unchanged platform build; no tenant-name execution branch | C01, C13, C16 |
| P02 | Home, click and slash provide predictable authorized navigation | Same stable destination; private suggestions absent; context switch discards late results; return restores work | C03, C11 |
| P03 | App and saved work are separate | Continue the same work versus start new work with fresh bindings/records; independent user reuse does not copy private data | C01, C02, C10 |
| P04 | Creation includes retrieval, adaptation and genuine new supported composition | Display which occurred; produce a new functional form/query/view and a persisted new app, beyond title/theme changes | C04, C08, C13 |
| P05 | CSV and bounded XLSX are inspectable inputs | Accepted/rejected counts, worksheet/mapping confirmation, units/time and row provenance; reject resource bombs/formulas outside policy | C05 |
| P06 | Queries are correct and bounded | Exact fixture totals before pagination; matching precedence, interval boundaries, currency/null semantics and receipts | C04, C06 |
| P07 | Forms and internal records support real work | Create/update persists after reload; schema/field authorization, audit, stale-edit conflict and timeout retry produce one intended mutation | C07 |
| P08 | Draft preview and transformation are useful and contained | Synthetic preview and isolated draft record writes; authorized real read-preview path; patch conflicts; reject unknown capabilities | C08, C12 |
| P09 | Publication follows organization policy | Exact-content admission, independent review in Northstar and a separately configured owner-publish test; author/model cannot change policy | C09 |
| P10 | Version changes preserve meaning | Existing work stays pinned, new work uses current allowed publication, explicit compatible adoption; incompatible schema blocked | C02, C10 |
| P11 | Access governs every information surface | Cross-tenant/source/record denial, filtered search, revoked saved narratives, queued-result recheck and export checks | C01, C06, C11 |
| P12 | AI assists within declared authority | Composer uses approved schemas/synthetic data; real-data explanation follows disclosure policy; injected evidence cannot expand tools or publish/write | C12 |
| P13 | Published work survives model outage | Navigation, deterministic queries and internal forms still work; generation fails visibly without changing published app | C14 |
| P14 | Work survives operational failures | Restart persistence, durable command outcome, restore exercise, import/query cancellation, deletion and quota evidence | C07, C15 |
| P15 | Platform generality is observable | Northstar six-card investigation plus Spend Review table/queue/detail/form, both created through the supported authoring path with no platform code change between them | C13, C16 |
| P16 | Canvas and accessible import remain central | Drop and Upload both create inspected source cards; move/resize/group/pan/zoom/fit, non-obscuring placement, keyboard/focused alternatives and saved layout survive reload; unrelated edits preserve work | C02, C03, C05, C08 |

P15 is a first-release gate, not a later optional expansion. A prebuilt second app may be useful setup, but loading it alone does not prove creation through Forma. Supported capabilities may be present in the library before the demonstration.

## Proposed planning envelope

These values make a small hosted pilot design estimable. They are **not supplied demand forecasts, an SLA, capacity certified on Forma 3.0, or founder-approved limits**. Recompute them in phase 1 against measurements and customer evidence. The smaller demo can retain the same contracts with lower volume.

| Assumption | Candidate value and derivation | If wrong |
|---|---|---|
| A1 Population | 10 organizations × 10 members = 100 seats; 3 active per organization = 30 concurrent users | Rework sessions, storage isolation, admission and cost before broader pilot |
| A2 Interactive load | One interaction per active user per 10 seconds = 3 interactions/s; six linked views imply up to 18 query operations/s; separately test 2 record submissions/s and simultaneous edits to one record | Add measured queue/concurrency controls; never relax authorization/conflicts to meet throughput |
| A3 Source volume | 20 new source revisions/org/month × 10 orgs × 25 MiB mean = 5,000 MiB ≈ 4.88 GiB original/month; normalized storage budget at 2× original ≈ 9.77 GiB; combined 14.65 GiB/month, about 43.95 GiB for 90 days | This excludes records, indexes, audit, backups and replication; measure each and revise costs |
| A4 Bounds | CSV ≤100 MiB and ≤1 million logical records; XLSX ≤10 MiB compressed, ≤100 MiB expanded, selected sheet ≤10,000 rows ×32 columns =320,000 cells; also bound archive entries, strings, parse time/memory | Reduce until demonstrated; compressed-byte limits alone are insufficient. Exact resource budgets are a phase-1/2 decision |
| A5 Admission | Initially one active import and generation job/org, three imports and five generations globally; cap queue depth and return visible busy/retry state | Avoid cross-tenant starvation with fair scheduling or separate workers; no silent job loss |
| A6 Data lifecycle for synthetic qualification | 90-day working retention and audit; seven-day backup window; idempotency outcome retained at least 24 hours | Real customer retention/deletion, retry horizon and legal obligations are open; do not inherit these test settings as company policy |
| A7 Recovery | Candidate pilot RPO ≤24 hours and RTO ≤4 hours, verified by restore; committed records durable across a normal process restart | Stronger customer requirements change backup/replication and operating cost; a backup timestamp alone is not restore proof |
| A8 Operating model | One managed browser offering; single-region initial deployment, manual membership administration permitted; named operational owner before live data | Region, identity/model providers, team, support hours, deployment date and budget are unknown; no old 30-day/$500 promise transfers |

Run latency trials at A1–A5 with declared dataset shapes and report p50/p95/p99, failure counts and admitted/rejected load. Candidate client-observed targets: authorized navigation p50 ≤250 ms/p99 ≤1 s; warm bounded query p50 ≤500 ms/p99 ≤2 s; internal record commit acknowledgement p50 ≤300 ms/p99 ≤1 s. Exclude neither authorization nor network time from these numbers. Report cold start/import separately. Generation is asynchronous with visible queued/running/failed state; measure time to an admitted draft instead of promising a fabricated model latency.

For a synthetic pilot, aim to update an already open display within 5 seconds of revocation under healthy connectivity; new server requests/delivery always reauthorize. This is a propagation test target, not recall of data already delivered. Do not issue a contractual availability percentage until the hosting model, support owner and failure measurements exist.

Cost is an explicit open decision. Measure app hosts, transactional store, source/query work, model input/output, retries, storage/backups and egress. Attribute generation spend by organization and stop at a configured budget. The factory must present a monthly cost model and operator effort; an unconstrained “cheap” claim is insufficient.

## Proof stages

1. **Stack/admission spike:** qualify the selected canvas/runtime code on released 3.0 using trusted renderers; persist and reload one typed app; reject executable/unknown output. Demonstrate query and record-command boundaries before polishing a film.
2. **Working dual-app demo:** pass the Northstar and Spend Review fixtures, reloads, version pins, policy checks, shared-work continuation, independent reuse and context return on one build. Record platform code revision, catalog versions, model/version, input hashes and every manual intervention. A fixture fallback is labeled and cannot stand in for a successful generation trial.
3. **Hosted pilot:** choose actual customer/data/provider/region/retention/owner; pass measured envelope, cross-tenant tests, restore and deletion. Rehearsal readiness is not permission to process customer evidence.

For AI evaluation, begin with a frozen small set: ten held-out supported intents split across investigation and record/form work, plus five unsupported/adversarial intents. Report retrieval/adaptation/generation outcomes separately; require all admitted results to validate, unsupported requests to abstain and every fixture-backed numeric answer to match its receipt. Report numerator/denominator and human corrections; a small set is diagnostic, not a universal reliability score. Expand it from observed use.

## Business learning and opinion

The platform story is stronger when the same user can make, change and share two different useful apps. The risk is breadth: a polished security app plus a hand-coded finance screen would still leave the platform claim unproven. Spend Review should stay compact and exercise capabilities Northstar does not, especially persistent internal forms.

Before calling this product validation, observe an external team completing recurring work, returning without founder assistance, handing work to a colleague and creating/adapting another supported app. Record baseline effort, elapsed completion time, correction rate, repeat usage and willingness to pay. A synthetic Northstar film and a simulated investor review establish neither customer demand nor actual YC feedback.

## Factory completion boundary

The preparation finishes when both inputs are preserved, source precedence and historical amendments are explicit, the shared contract is locked, both intakes are usable and review findings are closed or visible. The full design still requires architecture, diagrams, APIs, security/operations detail, alternatives, factory gates and editions. Do not run delivery or mark a design section reviewed from this packet alone.
