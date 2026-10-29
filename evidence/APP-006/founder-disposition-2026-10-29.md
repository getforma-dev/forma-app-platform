# Founder disposition of 2026-10-29: hosted identity rehearsal deferred; YC demo and live beta are the MVP

Recorded by the GenUI owner (Claude) from two sources on 2026-10-29: the FormaStack/fAuth owner's relay of Victor's decision given to it at about 18:40 UTC ("defer the six-hour joint rehearsal to the backlog, trust the qualified work, fail forward on bugs, and focus on the YC demo MVP"), and Victor's direct confirmation in the GenUI session ("Yes, defer and MVP"), followed by his clarification that the YC demo is the Northstar demo design (`design/forma-northstar-demo/deliverables/v1/md/design.md`, the frozen copy of the factory deliverable) and that, on top of the recorded demo, the deployment should be a usable beta that invited viewers can try themselves, "as real as close to the real app from the platform-v2 design", labeled beta.

## What the disposition decides

- The six-hour joint identity rehearsal, its browser matrix with the real callback, the deployed negative matrix, the KMS custody and A→B→C rotation campaign, the unknown-kid live refetch and the Canary-versus-Northstar resource check are deferred to the FormaStack backlog ticket FORMA-365 and, on the GenUI side, to APP-079 (PH06). They are not waived; they are required before any real organization signs in.
- FORMA-247 and FORMA-248 are Done by founder disposition on reviewed and merged source, CI and local/synthetic qualification. GenUI records their consumer gates as cleared by this disposition: the identity contract, verifier and callback are accepted as reviewed source; hosted proof is APP-079's.
- The mail verification follow-on is FORMA-366 (gatewasm PR 20, merged as `1bf5d3b`; the image is not yet qualified or deployed); the beta go-live trial APP-076 is gated on it, not on FORMA-247, because invited outsiders must be able to verify a mailbox to use the beta.
- GenUI's managed tenant qualification keeps what the beta needs (migrations, seed, the qualification entry run and the production HTTP surface on RDS) and defers the snapshot/restore drill, the two-process race coordinator on RDS and the drift/replica trials to APP-080 (PH05).
- No real organization or customer data may rely on this disposition. Synthetic data only; the beta is labeled beta; no SLA.

## What it does not decide (Victor's pending choices)

1. The always-on hosting budget for a live beta: the cost model prices GenUI's always-on synthetic environment at about $150–170/month and FormaStack estimates a lean fAuth deployment at about $33/month, against ADR-003's $100/month cap, which was set for a bounded qualification window.
2. The mail transport for signup verification (a verified sender on getforma.dev through fAuth's SMTP hook or Resend), which FormaStack configures and whose DNS records GenUI creates.

See [ADR-008](../../docs/architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md).
