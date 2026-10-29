# Founder disposition of 2026-10-29: hosted identity rehearsal deferred; YC demo and live beta are the MVP

Recorded by the GenUI owner (Claude) on 2026-10-29. Restructured the same day after the [independent review](adr-008-restructure-review.md) (finding M2) so that Victor's words and the teams' application of them are kept apart.

## What Victor said

1. Relayed by the FormaStack/fAuth owner, given to it at about 18:40 UTC: "defer the six-hour joint rehearsal to the backlog, trust the qualified work, fail forward on bugs, and focus on the YC demo MVP".
2. Direct confirmation in the GenUI session: "Yes, defer and MVP".
3. His clarification in the GenUI session, as GenUI recorded it at the time: the YC demo is the Northstar demo design (`design/forma-northstar-demo/deliverables/v1/md/design.md`, the frozen copy of the factory deliverable), and on top of the recorded demo the deployment should be a usable beta that invited viewers can try themselves, "as real as close to the real app from the platform-v2 design", labeled beta.
4. Later the same day, to the FormaStack session: "Approved: deploy the lean always-on fAuth (FORMA-367) at about $33/month using SES SMTP. Coordinate the timing with GenUI."

Nothing else in this record or in ADR-008 is Victor's wording.

## How GenUI and FormaStack applied it (owned by ADR-008, recorded for Victor's veto)

- FormaStack placed the deferred rehearsal content (its browser matrix with the real callback, the deployed negative matrix, the KMS custody and A→B→C rotation campaign, the unknown-kid live refetch and the Canary-versus-Northstar resource check) in its backlog ticket FORMA-365; GenUI placed its consumer side in APP-079 (PH06). Both teams treat the deferred proof as required before any real organization signs in; that gate is their reading of "defer … to the backlog", not Victor's words.
- FormaStack closed FORMA-247 and FORMA-248 as Done by disposition on reviewed and merged source, CI and local/synthetic qualification, and opened FORMA-365 and FORMA-366. GenUI cleared their consumer gates in its register on that basis: the identity contract, verifier and callback are accepted as reviewed source; hosted proof is APP-079's.
- FormaStack scoped the mail verification follow-on as FORMA-366 (gatewasm PR 20, merged as `1bf5d3b`; the image is not yet qualified) and split the lean always-on deployment into FORMA-367. GenUI gated the beta go-live trial APP-076 on FORMA-366 and FORMA-367 rather than on FORMA-247, because invited outsiders must be able to verify a mailbox to use the beta.
- GenUI defers its own snapshot/restore drill, two-process race coordinator on RDS and drift/replica trials to APP-080 (PH05) as its reading of "fail forward" and "MVP"; Victor did not name GenUI's managed-RDS qualification. GenUI's managed tenant qualification keeps what the beta needs (migrations, seed, the qualification entry run and the production HTTP surface on RDS). Until APP-080 exercises the restore, the snapshot is retained but unproven, and during the beta the only rollback for a committed migration is a reviewed forward fix.
- GenUI's guardrails: no real organization or customer data may rely on this disposition; synthetic data only; no SLA. Only "labeled beta" is Victor's.

## What it does not decide (Victor's pending choice)

1. GenUI's always-on hosting budget for the live beta: the cost model prices GenUI's always-on synthetic environment at about $150–170/month (Linux/x86 pricing; the ARM64 task costs less), against ADR-003's $100/month cap, which was set for a bounded qualification window and governs until he approves a figure.

The mail transport and FormaStack's lean fAuth deployment were answered by quote 4: GenUI reads "using SES SMTP" as the transport (a verified `getforma.dev` sender through fAuth's SMTP secret, configured by FormaStack; GenUI creates the DNS records).

See [ADR-008](../../docs/architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md).
