# FormaStack → GenUI dependency handoff

Owner assignment: October 28, 2026, at Victor's explicit request. **GenUI** is the app team in this repository. **FormaStack** owns the upstream framework and fAuth work. Assigning the team prepares parallel work; it does not claim a team has started or a message was dispatched.

The existing FORMA tickets remain authoritative in the FormaStack board. They are assigned to FormaStack and cross-linked to their consumers. No duplicate implementation ticket is created for an existing upstream fix.

| Upstream ticket / FormaStack owner | GenUI consumer | Required before | Unblock evidence |
|---|---|---|---|
| FORMA-246 — native composition conformance | [APP-003](../../Tickets/9-blocked/APP-003.md) | Native full-canvas qualification and integrated engineering-spike acceptance | Named composition fixture covers strict admission, typed Rust render, original-node adoption/interaction and refusal variants; reviewed/merged evidence and exact consumable version/ref. GenUI reruns its consumer checks |
| FORMA-232 — application budget resolution | [APP-003](../../Tickets/9-blocked/APP-003.md) | Full-library route-budget signoff | First measure under the shipped 3.2 caps. Either upstream configurable policy ships with evidence, or an explicit measured disposition proves this application needs no override and records the app target amendment. A disposition may clear this consumer while the broader FORMA-232 feature remains Backlog. No silent cap bypass |
| FORMA-247 — hosted fAuth deployment and platform client | [APP-006](../../Tickets/9-blocked/APP-006.md) | Integrated fAuth/session path | Current deployment revision, platform client/redirect configuration, PKCE back-channel flow, tenant-bearing claims, Northstar/canary isolation and rollback evidence. No passkey-button scope |
| FORMA-248 — production signing-key custody | [APP-058](../../Tickets/9-blocked/APP-058.md) | Real-organization sign-in and customer-data pilot | KMS-backed key custody, JWKS/key rotation overlap, verification during rotation and fail-closed outage/rollback evidence on the deployed identity service. Synthetic development may proceed before this gate |

See the [current handoff snapshots](../../Tickets/evidence/upstream-current/README.md) for assignments and original board paths. [Earlier snapshots](../../Tickets/evidence/upstream/README.md) preserve the pre-assignment record. The [machine-readable register](../../Tickets/external-dependencies.json) drives validation; snapshots do not become a second editable upstream board.

## Parallel work

FormaStack can take FORMA-246, the FORMA-232 disposition, FORMA-247 and FORMA-248 concurrently where its own dependencies permit. The fAuth work remains in its owning repository under FormaStack coordination.

GenUI starts APP-001 and contracts next, then follows the actual graph. Synthetic fixtures, parser confinement, query measurements, scaffold and tenant schema can move independently of hosted-auth/native acceptance where their listed predecessors permit. A whole sprint is not blocked merely because one consumer is blocked.

APP-003, APP-006 and APP-058 start in `Tickets/9-blocked/`. Their downstream tickets wait in Backlog with explicit dependencies. This prevents selection of dependent work without inflating every waiting task into a separate blocker.

## Handoff completion

1. FormaStack updates the existing FORMA ticket with exact commit/release/deployment evidence, checks run, limitations and rollback guidance. Source merge and registry/deployment availability are different facts.
2. GenUI records the receipt in the consuming APP ticket and runs its integration checks using the exact artifact. For FORMA-232, record whether the feature shipped or the specific consumer no longer needs it.
3. Update the external register's gate state to `cleared` only with that receipt. Move the APP file to Backlog or Selected according to its remaining dependencies and repair links.
4. Regenerate and validate the board. Never close a blocked APP ticket merely because the upstream work was assigned or the framework version increased.

Actual product choices, access or spending decisions are recorded separately. They are not automatically routed to Victor through the cross-team blocker folder.
