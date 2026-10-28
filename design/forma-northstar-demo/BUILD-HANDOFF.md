# From the demo design to one platform implementation

## Dependency and contract ownership

The [platform v2 task](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/README.md) owns the generic model and authority boundaries. This task owns the demonstrable journey, fixture expectations and reduced deployment. Adopt the same contract edition recorded by `platform-contract-lock.json`. If either design changes it, reconcile both before implementing dependent behavior.

The app specification remains a proposal: compare the typed `AppDefinition` interpretation model with the historical admitted-markup model in the factory. Do not build one for Northstar and another for Spend Review. Resolve publication policy, identity integration, source storage, transactional record storage and generated-output admission once for the pair. Existing platform v4 decisions are evidence requiring explicit amendment where they differ, not silently inherited implementation.

## Stack and prototype boundary

Use the released Forma 3.0.0 contract as the framework baseline, at the pin in [ALIGNMENT.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/ALIGNMENT.md). Its release does not establish that the app platform, ingestion service, catalog or record service already exists. FMIR build inputs are trusted; arbitrary generated FMIR is not a shipped admission path. Native SSR must preserve its full response/security contract; the app canvas may use an explicit browser route.

Vector Studio, Generative UI and GenUI Runtime provide interaction and composition prior art. They use older Forma dependencies and do not supply production identity, permissioned data execution or durable internal commands. Port and validate selected pieces; do not equate a canvas snapshot with authorized saved work. Proposed Forma 3.1 features are not available merely because an earlier design depends on them.

The domain library can register a versioned CIDR inventory lookup and explicit currency operations. Once that supported library is installed, both examples must be created by user-level composition and transformation. A platform branch on `Northstar`, `Security` or `Finance`, or a developer patch for the second example, fails the agnostic-platform proof.

## Contract-to-demo trace

| Shared contract | Demo evidence |
|---|---|
| C01 ownership and identity | S01 two organizations; S06 guessed/cross-tenant IDs denied |
| C02 definitions and stored work | S04 shared case versus fresh independent case; S05 separate record collection |
| C03 navigation and discovery | S01 click/slash identity; S05 return state and late-response isolation |
| C04 supported language | S01 meaningful composition; S02 exact bounded calculations; S05 typed form |
| C05 source interpretation | S02 mappings, window and match rules; S05 integer currency; S06 parser rejection |
| C06 query and provenance | S02 receipts, full-filter totals, linked epochs; S05 record snapshot sequence |
| C07 internal mutation | S05 commit, replay, conflict, version and audit; S06 uncertain outcome recovery |
| C08 preview and transformation | S03 semantic patch, stale patch conflict, isolated preview records |
| C09 publication policy | S03 exact revision review and policy-specific owner/reviewer behavior |
| C10 work-version adoption | S04 pinned cases, explicit adoption, blocked incompatible migration |
| C11 sharing and revocation | S04 current authority for raw and derived work; S06 delayed-result denial |
| C12 AI authority and disclosure | S06 server-enforced denied operation and unconfigured disclosure behavior |
| C13 library and domain meaning | S02 inventory operator; S05 second app through the same library |
| C14 external actions/surfaces | S05 internal decision produces no payment; browser/model-outage operation |
| C15 operations and retention | S06 bounded resource use and recovery; rehearsal reset and restore evidence |
| C16 demo compatibility | S07 same build, schemas and interfaces, second tenant canary and honest simulation labels |

Scenario definitions are in [SCENARIOS.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/SCENARIOS.md). This is a requirements trace, not a completed test report.

## Legitimate reductions and extension seams

| Demo reduction | Required seam for the platform |
|---|---|
| Two synthetic organizations and manually provisioned accounts | Real authenticated membership and tenant-bearing keys; replace provisioning without changing authority semantics |
| One operated deployment with small stores | Generic service interfaces; preserve transaction and parser/query isolation boundaries when splitting deployment |
| Small CSV/XLSX fixtures | Same bounded import and mapping pipeline; increase only measured limits |
| Two apps and a small trusted component library | Versioned registry and admission; new user definitions need no platform deploy |
| One configured model or synthetic composition fixtures in an early spike | Provider adapter and disclosure policy; fixture generation is labeled and cannot pass the working generation gate |
| Manual policy/retention administration | Versioned policy and audited admin interface; no author-controlled grants |
| No simultaneous coediting | Durable shared work, expected versions, visible conflicts and explicit refresh |
| No live connector or external action | Source/command interfaces preserve the distinction between internal records and future external effects |

Lower scale and manual administration are reasonable. Replacing authentication with an organization dropdown, storing all live work only in browser memory, faking record commits or using hardcoded screens for Act 6 are not reductions that satisfy the claimed capabilities.

## Sequence and exit evidence

**Design entry:** use the two preparation packets, record accepted/amended shared contracts, choose the app representation and storage/identity approach, and estimate the work. No delivery date, staffing or operating budget is agreed here.

**Engineering spike:** prove one complete path: admitted definition → authorized query → visible receipt → explicit internal command → transactional outcome/audit → reopen. Exercise duplicate submission and version conflict. Show a bounded XLSX import and prototype-to-3.0 component integration. This resolves architectural uncertainty before polishing six acts.

**Working demo:** complete S01–S07 on one build. Create Spend Review through the same supported authoring path; demonstrate Sarah's review, Jordan's independent reuse, restore after reload and denial after revocation. Rehearse the six acts and capture both success and recovery evidence. A recording may support the presentation, but the evidence comes from the running path.

**Pilot entry:** choose a real repeated workflow and owner, confirm data/model disclosure and retention, set measured capacity/latency/recovery targets, verify restore and operating responsibility, and rerun permission boundaries with realistic membership. Synthetic demo completion does not waive these decisions. The [platform validation plan](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/VALIDATION.md) owns shared numerical targets and pilot readiness gates.

Suggested demo input limits are 10 MiB/100,000 logical CSV rows and, per selected XLSX sheet, 5 MiB compressed, 50 MiB expanded, 10,000 rows and 32 columns. These are proposed ceilings for parser validation, not measured promises. Enforce expanded archive and workbook-wide resource budgets before materialization, even when only one sheet is selected. The sample fixtures are far smaller.
