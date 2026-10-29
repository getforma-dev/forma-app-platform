# Identity rehearsal on the current schema1 foundation

The separate identity-only auth rehearsal does not depend on APP-008's version2 tenant migration. The current database can remain at its qualified version1 foundation while the already reviewed callback consumer runs. This sequencing correction supersedes the earlier coordination message that made a new schema2 callback deployment a precondition. FormaStack acknowledged the corrected separation; neither side has issued a start signal.

## Exact artifacts and scope

Use callback source `18c33f538a19d06cb77f37d091c71b1a8d5e3c39`, whose eight successful CI checks, private secret handoff and local image review are retained in [secret-wiring](../secret-wiring/README.md). Its immutable registry index is `sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08`, ARM64 child `sha256:08d350fd186f72886823c9b33be21b1df1a70d4f96b1a35e63d639fb7c775ac9` and config `sha256:d306df2f340a77a2c4cb95847fc7f8237eea29e16118780a7ee056739b817103`. Root inspected that exact committed runtime: its helper requires schema1 and the four fixed IAM role identities; it does not implement APP-008's new native pools or canonical catalog guard.

The separate one-shot canonical key qualifier remains source `8fca35c35b80828d4effb17e59f9bbd80ae237ea` and [its own registry image](../../hosted-key-qualification/registry-binding.json). Keep both source/image pairs distinct in operator configuration and evidence. The qualifier cannot be used as the HTTP service image.

Callback success yields an external identity observation only. It grants no membership, entitlement, app session or private data access. No verified-mailbox activation is inferred from the no-SMTP synthetic rehearsal. The origin remains `https://beta.getforma.dev` and exact callback `https://beta.getforma.dev/auth/callback`.

## Fresh observations and remaining launch work

[Control-plane inspection](control-plane.json) verified the original protected RDS resource at PostgreSQL16.13, service desired/running/pending counts all zero and no cluster tasks desired RUNNING/PENDING. Root re-fetched and hashed both callback registry manifests and verified the ARM64/config binding. The subsequent [managed SQL preflight](sql-preflight/managed-observation.json) passed the exact version1 catalog before metadata, operator/origin checks and verified TLS in a read-only transaction. The task ran once, stopped after 53.012 seconds and its temporary definition was deregistered; follow-up service/task counts remain zero. This point-in-time observation relies on preserving schema/operator quiescence before and throughout the rehearsal.

The [six-hour cost forecast](budget-forecast.json) applies the reviewed preflight calculation to an in-memory ledger copy. It projects $98.59 including full-month retained infrastructure, all prior reservations, the proposed GenUI compute allowance, existing variable reserve, the auth $5 counted once and $25 unallocated. The actual ledger was not changed. This is neither a selected interval/reservation nor a provider billing cap; the real immutable input, secret ARN, current ledger and future stop schedule must be checked and reserved when both sides are ready.

Before activation, root must verify that the observed version1 baseline has remained quiescent, finish review of the bounded configuration/secret handoff, complete the exact count-zero stack update and inspect the actual task definition, conditional execution-role policy, ingress/TLS and future stop schedule. Any intervening operator/schema change invalidates the SQL observation and requires fresh qualification. Provider IDs and private configuration follow the agreed provisioning sequence inside the joint window; their absence does not block this pre-start preparation. Rust still validates the secret's exact closed configuration and expiry before listening.

Safari capability confirmation and both teams' readiness remain required. No additional budget decision is requested. The actual interval starts only after the agreed coordinated signal, with the approved six-hour maximum and scoped cleanup.

The [callback activation runbook](activation-runbook.md) now has [independent operational review](activation-independent-review.md), with no blocking finding in the prepared manual procedure. It fixes two deliberate process segments: three serial existing-Maya browser logins, then one separate fresh invited signup, with all old tasks observed stopped before changing the secret version. Private version/configuration binding is operator evidence, not runtime attestation. Unexpected process replacement aborts the campaign; the original deadline is never renewed. Actual provider confirmation, Safari capability and concrete inside-window requests remain required before execution.

[Fresh no-lookups synthesis](fresh-synthesis.json) passes using infrastructure source identical to the already accepted revision. Its 84,297-byte template (52,793 compact) requires a reviewed TemplateURL upload rather than TemplateBody. No upload, new reservation, secret creation, count-zero stack update or activation occurred. The actual artifact destination, upload hash and change-set review belong to the concrete provisioning sequence.

## Exclusion and shutdown boundary

Keep every APP-008 managed migration, seed, drift test, operator mutation and restore action outside the auth window. Its local source, review, image and CI work may continue in parallel. Preserve the approved version1 database and prevent concurrent operator/schema changes throughout identity qualification; the old callback is not claimed to have the later catalog protections or to pass APP-008.

Collect the sanitized identity results and reconcile provider cleanup, then stop the callback and all owned one-off tasks and observe zero running/pending consumers before any version2 migration. A later version2 database requires its matching reviewed runtime and its own managed acceptance. Changing back to a version1 image while leaving the database at version2 is not rollback. APP-006, APP-008 and the public beta gates remain open until their own evidence is accepted.
