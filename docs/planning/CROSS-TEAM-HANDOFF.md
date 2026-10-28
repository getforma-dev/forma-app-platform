# FormaStack → GenUI dependency handoff

Owner assignment and execution handoff prepared: October 28, 2026, at Victor's explicit request. **GenUI** is the app team in this repository. **FormaStack** owns the upstream framework and fAuth work. The separate **Complete Forma auth tickets** chat has claimed the framework/auth lanes; a coordination update was sent on October 28 with current GenUI progress, repository ownership and GateWasm rebuild prerequisites. Assignment and local implementation do not clear consumer gates.

The existing FORMA tickets remain authoritative in the FormaStack board. They are assigned to FormaStack and cross-linked to their consumers. No duplicate implementation ticket is created for an existing upstream fix.

| Upstream ticket / FormaStack owner | GenUI consumer | Required before | Unblock evidence |
|---|---|---|---|
| FORMA-246 — native composition conformance | [APP-003](../../Tickets/9-blocked/APP-003.md) | Native full-canvas qualification and integrated engineering-spike acceptance | Named composition fixture covers strict admission, typed Rust render, original-node adoption/interaction and refusal variants; reviewed/merged evidence and exact consumable version/ref. GenUI reruns its consumer checks |
| FORMA-232 — application budget resolution | [APP-003](../../Tickets/9-blocked/APP-003.md) | Full-library route-budget signoff | First measure under the shipped 3.2 caps. Either upstream configurable policy ships with evidence, or an explicit measured disposition proves this application needs no override and records the app target amendment. A disposition may clear this consumer while the broader FORMA-232 feature remains Backlog. No silent cap bypass |
| FORMA-247 — hosted fAuth deployment and platform client | [APP-006](../../Tickets/9-blocked/APP-006.md) | Integrated fAuth/session path | Current deployment revision, platform client/redirect configuration, PKCE back-channel flow, tenant-bearing claims, Northstar/canary isolation and rollback evidence. No passkey-button scope |
| FORMA-248 — production signing-key custody | [APP-058](../../Tickets/9-blocked/APP-058.md) | Real-organization sign-in and customer-data pilot | KMS-backed key custody, JWKS/key rotation overlap, verification during rotation and fail-closed outage/rollback evidence on the deployed identity service. Synthetic development may proceed before this gate |

See the [current handoff snapshots](../../Tickets/evidence/upstream-current/README.md) for assignments and original board paths. [Earlier snapshots](../../Tickets/evidence/upstream/README.md) preserve the pre-assignment record. The [machine-readable register](../../Tickets/external-dependencies.json) drives validation; snapshots do not become a second editable upstream board.

## Parallel work

FormaStack can take FORMA-246 and the FORMA-232 disposition together in the framework lane, with FORMA-247 in an independent fAuth lane. FORMA-248 follows its existing FORMA-247 prerequisite; it must finish before real-organization sign-in. The fAuth work remains in its owning repository under FormaStack coordination.

GenUI has completed APP-001/002/004/005, with independent reviews and passing CI. APP-007 has reviewed local preparation and passing CI but remains Blocked for deployment inputs and managed trials. Subsequent work follows the actual graph. Synthetic fixtures, parser confinement, query measurements, scaffold and tenant schema can move independently of hosted-auth/native acceptance where their listed predecessors permit. A whole sprint is not blocked merely because one consumer is blocked.

APP-003, APP-006 and APP-058 start in `Tickets/9-blocked/`. Their downstream tickets wait in Backlog with explicit dependencies. This prevents selection of dependent work without inflating every waiting task into a separate blocker.

## Ready-to-pick-up instructions

Each authoritative FORMA ticket has a **2026-10-28 execution handoff** section with ordered implementation steps, required checks, exact handback evidence and ownership boundaries. The FormaStack agent has claimed the work. At the latest coordination check, FORMA-232 was Done, FORMA-246/247 In review and FORMA-248 In progress; consult the original board for later moves. FORMA-232's configurable-budget source is merged but its package is unreleased. APP-003 now prepares the full-library consumer fixture using registry 3.2.0; only its actual shipped-cap measurement can establish a no-override disposition. No app gate is cleared by the upstream status alone.

| Start order | Authoritative local ticket | Writable ownership |
|---|---|---|
| Framework lane: start together | `/Users/victorvillacis/dev/Forma/Tickets/3-in-review/FORMA-246.md`; budget source receipt in `/Users/victorvillacis/dev/Forma/Tickets/4-done/FORMA-232.md` | Isolated checkout of `getforma-dev/forma`; compiler/native fixture and route-budget code/tests/docs |
| Auth lane: parallel with framework | `/Users/victorvillacis/dev/Forma/Tickets/3-in-review/FORMA-247.md` | Isolated checkout of `getforma-dev/gatewasm`; fAuth and designated auth environment |
| Auth custody: integration after FORMA-247 | `/Users/victorvillacis/dev/Forma/Tickets/2-in-progress/FORMA-248.md` | Same coordinated fAuth lane; KMS/JWKS/managed-secret changes |

The framework reference checkout is `/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma`; the fAuth reference checkout is `/Users/victorvillacis/dev/FormaBuiltApps/gatewasm`. These are discovery references, not permission to overwrite another working tree. Read each owning repository's current instructions and inspect the worktree before selecting an isolated work context. The published framework baseline remains 3.2.0, tag `fc76ea8531f0e3a85d0224087384e41f52040e3c`; the observed fAuth source pin is `b750ca534ae89066714f100eb2b76ee71edf1ab1`, not a claim about the live deployment.

GenUI owns this app repository, its APP tickets, schemas, lockfiles, application fixtures and consumer verification. FormaStack should not edit this checkout or its gate register. In particular, deliver any demo provisioning script as a reviewed patch/artifact for GenUI to incorporate; coordinate app origin/redirect values instead of guessing them. No team should reset the other's data, terminate its processes, migrate donor apps, or rewrite frozen design/upstream snapshots.

Before starting, the FormaStack agent records its name, ticket, branch/worktree and files/services owned in the original ticket. When ready, it reports the exact commit/PR, released package or deployed image identity, reproducible commands and results, independent review/CI, limitations and rollback. GenUI then records receipt and consumer acceptance. New contract changes or operational blockers should be reported as soon as discovered. This file and the original ticket Logs are the durable coordination record. The October 28 coordination message to chat `01a18408-d9ee-7501-98ba-83b062f4d121` supplements these instructions; it requests no GenUI source or contract changes from that lane.

The original upstream board's older clone, attribution and release instructions are historical context. Current user authorization and owning repository instructions govern execution; this handoff alone publishes no package and deploys no service. Missing environment access or a concrete spending decision is recorded against its specific operation, while independent local work continues. If FORMA-248 rolls back to a process-readable file key, real-organization sign-in becomes blocked again even if synthetic login works.

## Handoff completion

1. FormaStack updates the existing FORMA ticket with exact commit/release/deployment evidence, checks run, limitations and rollback guidance. Source merge and registry/deployment availability are different facts.
2. GenUI records the receipt in the consuming APP ticket and runs its integration checks using the exact artifact. For FORMA-232, record whether the feature shipped or the specific consumer no longer needs it.
3. Update the external register's gate state to `cleared` only with that receipt. Move the APP file to Backlog or Selected according to its remaining dependencies and repair links.
4. Regenerate and validate the board. Never close a blocked APP ticket merely because the upstream work was assigned or the framework version increased.

Actual product choices, access or spending decisions are recorded separately. They are not automatically routed to Victor through the cross-team blocker folder.

## Current hosted-auth operating state

2026-10-28: At Victor’s explicit request, GenUI is pausing unused GateWasm AWS resources to reduce charges, preserving data. FORMA-247/248 now carry the stop/coordination notice. FormaStack should continue isolated local work and coordinate the next hosted-auth test window before restarting these services. Auth gates remain open; the shutdown is not delivery evidence.

2026-10-28 update: the user subsequently authorized full GateWasm application teardown because it has no real data. This supersedes the temporary-pause plan above. GenUI owns teardown; FORMA-247/248 carry the current coordination notice and remain open for later reprovisioning and qualification. CDK/source and shared bootstrap are preserved.

Teardown completed at 2026-10-28 19:29:59 UTC (four deleted application stacks, 27 final checks). FORMA-247/248 now include exact rebuild prerequisites and the external-secret recovery window. The GenUI report is `evidence/APP-007/gatewasm-teardown.md`; no hosted auth availability is claimed.
