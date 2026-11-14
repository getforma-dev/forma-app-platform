# ADR-004: Tenant transactions, pool ownership and foundation delivery

Accepted implementation direction, 2026-10-29. This resolves the [APP-008 planning review](../../evidence/APP-008/planning-review.md); it is not implementation or test evidence. [APP-008](../../Tickets/4-done/APP-008.md) owns the complete tenant foundation. Platform §6.3, C01/C07/C11 and ADR-003 remain controlling. Later session, admission, publication and command workflows retain their own gates.

*Amendment 2026-10-29 ([ADR-005](ADR-005-PHASE-1-SEQUENCING.md)):* slice 3's actual PostgreSQL/RDS isolation, reuse, race and recovery qualification is now owned by APP-075/077/078; APP-008 closes on slices 1–2 plus exact-source CI and landing. Every managed obligation below is unchanged and PH01 still cannot pass without it; the text is retained as the original decision.

## Delivery and roles

Keep APP-008 as three separately reviewable implementation slices: checksummed schema/version transition; native pools and transaction authorization; actual PostgreSQL/RDS isolation, reuse, race and recovery qualification. Its estimate remains the upper end of L with parallel authors. Reassess if the concrete implementation exceeds five focused days; no slice alone closes the ticket.

Implement the real relations in [schema-plan.md](../../evidence/APP-008/schema-plan.md), including publications and records with their real tenant-bearing references. No test-only replacement tables. Operator-seeded contract rows must validate against APP-002; use newly validated fixture envelopes for the actual managed organization IDs, preserving the original specimens. Initial application grants are scoped reads and the explicit functions below. Do not grant future publication, record, session, job or entitlement writes before the owning transaction/audit implementation. Report read isolation and write refusal precisely; do not claim successful application writes from this slice.

Preserve workspace/control/data/worker as four fixed identities. Worker initially has the limited organization/operational reads needed to prove its real RLS boundary; job execution authority comes later. Workspace, control and data receive the specific member-fence function; only data receives the work-fence function until another implemented caller requires it. Every granted tenant relation has ENABLE/FORCE RLS, tenant-leading keys/FKs and both USING/WITH CHECK. Missing or malformed scope never grants access.

## Request and operation states

| State | Allowed behavior and transition |
| --- | --- |
| Trusted request identity | Created by the internal adapter after actual verification and server mapping. Publicly constructible verifier output, request headers and provider roles are insufficient. Ordinary production session/admission remains unavailable until APP-009/073. |
| Private transaction | BEGIN READ COMMITTED and install bound transaction-local org/principal/epoch. Pool checkout itself takes no membership lock. No raw client, SQL API, stream, transaction or background operation escapes the storage module. |
| Current precheck | After installing trusted scope in that transaction, read current organization/member/resource permissions on the primary connection. No unscoped membership lookup occurs. This is not a reusable grant or authority to commit. |
| Operation fence | For present reads, take the required authority fence then evaluate current permissions/resource scope. Future APP-013 commands preserve current precheck → scoped outcome → authority fence → saved work → record. An outcome replay still requires current visibility before delivery. |
| Transaction-bound permit | Private, non-cloneable and tied to this transaction/action/resource. It cannot survive rollback or be reused by another operation. No savepoint API is exposed in this slice. Future savepoint support must invalidate/reacquire affected permits. |
| Finish | Await commit or explicit rollback, then standalone reset and verification. Only confirmed cleanup permits reuse. Cancellation, ambiguous commit or failed cleanup discards; it never causes an automatic business retry. |

Read-only business operations use a read-write SQL transaction when their fence needs row locks. They gain no business DML privilege from doing so.

## Exact fence ownership

Create a distinct `genui_lock` NOLOGIN/NOINHERIT/NOBYPASSRLS principal, with no IAM membership and no application role membership in it. It owns only fixed, schema-qualified lock/read functions. Grant the minimum SELECT columns and one lock-enabling UPDATE column on organizations, memberships and saved work; no other table/sequence privileges, no final schema CREATE, no generic SQL or mutation function. Temporary DDL privileges, if PostgreSQL requires them for ownership transfer, must be revoked and independently inspected. The all-schema owner is not the function definer.

`genui_private.lock_member_authority(uuid, uuid, bigint)` checks supplied org/principal/epoch against transaction context, locks the organization FOR SHARE then that membership FOR SHARE, and returns only bounded active/epoch facts. Workspace/control/data may execute it. `genui_private.lock_work_revision(uuid, uuid, uuid, uuid, bigint)` checks scoped org/app/work/publication/version and locks only that work FOR SHARE; data may execute it. Both retain owner FORCE RLS, safe fixed search path, revoked PUBLIC execution and no scope-setting or privilege-changing behavior. A row-locking definer's privileges must never become direct application UPDATE grants.

Authority writers must participate in the same fences. Organization suspension takes the organization FOR UPDATE. Membership state or any existing per-principal app/work/source/organization permission change takes organization FOR SHARE, then the member FOR UPDATE, and advances that member's epoch in the same transaction. No direct application grant-table mutation is permitted in this slice. Operator qualification uses the same reviewed transaction ordering. Current authorization checks organization state, epoch, action and actual resource relationships; delivery rechecks remain required.

Publication policies/revisions are immutable in this slice. Later retirement/policy/adoption implementations must name and lock their additional mutable premise rows before acquiring write grants. APP-073 must add entitlement/code expiry and revocation to admission, commit and delivery checks, using the same organization/member fence where it changes effective membership. It must expose a separate private same-transaction admission path for a verified nonmember; it must not require an existing-member permit or add a generic bypass flag. APP-008 does not pretend these future premises already exist.

## Pool and driver lifecycle

Use the proposed pinned native dependencies only after resolving registry/lock/MSRV checks. Maximum two connections per fixed role and eight total connecting/live driver slots. A driver owns its connection-capacity permit until transport closure; cancellation must not release capacity merely because abort was requested. Encapsulate this ownership and test cancellation during connect/recycle as well as queries. Bound queued acquisitions separately; do not allow an unbounded waiter queue.

Use one five-second absolute acquisition deadline, including queue, connection, TLS, identity/schema checks and at most one changed-generation connection retry. A transaction has a three-second maximum and ends before the credential-file expiry minus a safety margin; SQL statement/lock/idle limits are subordinate. Cleanup/reset has a separate two-second bound. These are foundation operating bounds, not proof of the design's latency targets. No mutation is automatically retried.

Every new connection uses the current fixed-role IAM credential and packaged RDS CA with hostname verification, PostgreSQL TLS negotiation and no plaintext/insecure fallback. Verify actual user, flags/memberships and supported schema before admitting it. Every checkout requires valid current configuration; an older-generation idle connection is retired. An in-flight lease may finish only within its original bound and is not reused under a newer generation.

Leases default to non-reusable. Successful finish requires awaited transaction termination, standalone DISCARD ALL, baseline reinstallation and explicit user/role, row_security, search_path and blank scope checks. No statement cache survives server reset. Uncertain leases synchronously detach their pool object; their owned driver must close. Only fully verified leases return to the pool. Tests must prove same-backend reuse after success/rollback and a new backend after uncertain cleanup, including poisoned session context and cancellation.

## Identity registry and qualification seam

Use a separate closed, private registry file, never an undeclared extension of the existing credential/callback JSON: `schema_version:1`, `registry_version` UUID and at most32 exact `{issuer, provider_tenant_id, organization_id}` bindings. Require owned private regular file/directory, no links, bounded16KiB input and unknown-field rejection. Each exact provider pair maps to one organization; multiple separately approved pairs may map to one organization. Read once and retain its SHA-256/version. Scoped DB binding confirmation must agree. No unscoped application lookup or prefix stripping.

Registry replacement requires process restart and explicit session invalidation/reconciliation; future APP-009 sessions bind the registry identity. Fixture identities are confined to the isolated qualification/test build, with no production fixture constructor, enabling header or alternate login endpoint. Real identity/session/entitlement journeys remain APP-006/009/073/074. Qualification may use a separate test executable compiled from the same production modules, with its source/image identity recorded; it cannot be represented as a production login.

Pre-integration startup may use an explicitly empty binding list. It permits fixed-role database readiness only; every identity/resource lookup remains denied. The managed launcher creates a new private empty registry when no operator registry path was supplied, and removes only its own file on shutdown. It must never fabricate provider identities or replace an operator registry. An absent registry in a directly launched binary keeps the public inert shell available but readiness false; an explicitly supplied invalid registry refuses startup. Native four-role readiness and the existing S3/KMS probe must both pass for the same current credential generation. Real provider bindings and session invalidation remain the later identity/session integration gate.

## Migration and recovery

Preserve migration0001 bytes/history. Add checksummed migration0002 and exact supported database version2 checks in the runtime/native readiness and migration/restore qualification. Bootstrap a migration history only after verifying the known version1 foundation; refuse incompatible versions, altered checksums or partial state. Apply migration2 transactionally and prove idempotence. No automatic destructive downgrade exists: stop incompatible service, use the reviewed prior snapshot/compatible image or a reviewed forward fix. Root alone operates the managed environment after source/review/CI and a bounded reservation; schema changes must be coordinated with the auth rehearsal.

The logical catalog must accommodate the observed provider difference in PostgreSQL creator bookkeeping: optional incoming grants from `genui_migration` to one of the six exact fixed roles with `admin=true`, `inherit=false`, `set=false` are excluded from comparison. The accepted APP-007 source/restored manifests omit these local creator grants. Preserve the explicit migration-to-owner grant and every other membership/flag, all fixed-role outgoing reachability and unrecognized incoming members. This narrow normalization changes no grants and is not permission to learn an expected baseline from a running database; fresh managed compatibility still must be checked. Establish a fixed `pg_catalog` search path before using unverified metadata, and validate catalog structure before reading application-owned version/history relations. Replica mode and grants that let an application role enable it must be refused; no privileged repair is an availability fallback.

Acceptance still requires real per-role/two-tenant catalog/FK/RLS tests, same-connection cleanup, two-order revocation races, uniform private-route refusals and managed runtime evidence. This decision supplies none of those results and does not pass Phase 1.

## Amendment A5, 2026-11-14: the catalog comparison on reuse runs at most every 30 s (APP-197)

Decided by the Fable advisor in lieu of Victor (his delegation of 2026-11-14), for [APP-197](../../Tickets/0-backlog/APP-197.md), after the [APP-195 refresh-herd campaign](../../evidence/APP-195/refresh-herd-2026-11-14.md). ADR-007's amendments A1–A4 come first; this is A5. Revised the same day after the [Tier A review](../../evidence/APP-197/review-tier-a-2026-11-14.md) (M1, m1, m2, n1, n2). Victor can reverse it.

**What it amends.** §Pool and driver lifecycle: "Only fully verified leases return to the pool." A reused lease is now fully verified except for the structural catalog comparison, which it repeats at most every 30 s.

**Why.** Every lease reuse ran the full structural catalog comparison. That comparison is one query that builds the whole catalog in PostgreSQL, plus its comparison in the service. Measured locally, it took about 45 ms of a 64 ms data-role transaction, and every reuse ran it: 20 checks in 20 transactions. One page refresh takes 12 data-role and 5 workspace-role transactions, so a 26-page refresh herd needed about 10 s of data-role time on two connections. Neither the 32-caller waiter set (APP-195) nor four data connections made it pass.

**Decision.**

- **A new connection** runs `verify_identity`, the catalog comparison, the version relation's identity check and `verify_schema_version` before it is admitted. **Readiness** (`Stores::readiness` and its diagnosis) runs the same on every probe. Both are as before, plus the identity check.
- **A reused connection, on every reuse,** runs in this order:
  - `verify_identity`: flags, memberships, TLS, replication role and `search_path`;
  - the version relation's identity check, from `pg_catalog` alone: `genui_meta.schema_version` is still the relation, by OID, that the connection's last comparison saw; it is a plain table without row security, with a `boolean` and an `integer` column;
  - `verify_schema_version`, now `FROM ONLY`;
  - `verify_baseline` last: user, role, `row_security`, `search_path`, blank scope and replication role. Nothing those reads ran can leave the session changed without being seen.
- **The catalog comparison on reuse** runs only when the connection's own last comparison is at least `CATALOG_RECHECK` old. That is 30 s, a `const` in `storage/pool.rs` with a compile-time assertion of its value, and no environment or configuration override. The window is measured from the instant before that comparison began, and a reuse that does not compare never moves it.
- **A failed check on reuse discards the connection, as before.** The driver closes it and its capacity is released.
- **The test executable** can move the pool clock forward (`TestHooks::advance`). No production path can.
- **Refusal causes stay typed `trace::refuse` codes.** The new ones are `version_relation_missing`, `version_relation_shape` and `version_relation_changed`.

**What moves to the 30 s cadence:** the catalog document, when it changes without a schema-version bump. That covers grants and ACLs, relation kinds, RLS policies, triggers, constraints, functions and role attributes.

**What stays checked on every reuse:**

- identity and the replication role, so replica mode is caught at once;
- the schema version, so a migration that bumps it is refused on the next reuse;
- the version relation's identity. §Migration and recovery's rule, "validate catalog structure before reading application-owned version/history relations", therefore still holds inside the window. If the relation is replaced by a view, or given row security whose policy runs code, the reuse refuses before anything in it runs. The APP-008 native campaign asserts this inside the window, with zero marker executions.

**Threat model.**

- The comparison defends against operational drift of the database the service runs against: a migration applied while the service runs, grant or flag differences, replica mode, or a relation renamed away or replaced.
- It is not a defence against a hostile database administrator. The [APP-008 r5 review](../../evidence/APP-008/native/r5/independent-review.md) already says: "No protection against a hostile database administrator changing DDL after validation is claimed."
- No request path can cause catalog drift. The storage module issues only fixed statements, and the definer functions contain no DDL.
- The four application roles own nothing: no schema, relation, function, type or large object. They hold no CREATE or TEMP privilege, and none is SUPERUSER, CREATEDB or CREATEROLE. The APP-008 native campaign asserts all of this.
- PostgreSQL still lets each role set its own per-role defaults (`ALTER ROLE <self> SET …`), and the catalog compares those rows. They apply only to new connections, which are always compared before they are admitted.

**Consequence.**

- For catalog drift on a reused connection, detection moves from the next transaction to at most 30 s, plus the transaction in flight (at most 3 s). A new connection, readiness and every per-reuse check detect it as before.
- Inside the window, a reused connection can run statements against drifted objects other than the version relation: a loosened policy, a dropped constraint, a redefined definer function. That is the same exposure as any statement a transaction runs after a successful check, now for up to 30 s instead of one transaction.
- No connection count, deadline or waiter bound changes. ADR-007's numbers are unchanged.
