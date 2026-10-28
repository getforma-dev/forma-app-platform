# ADR-002 — Freeze the first implementation contract edition

Date: 2026-10-28. Owner: GenUI. Ticket: APP-002. Edition: `impl-2026-10-28.1`. This decision supplies implementation contracts and test vectors; it does not implement admission, authorization or persistence.

## Source authority and representation

Both the platform and Northstar demo inherit shared contract edition `prep-2026-10-22.1`. Preserve the source copies and companion lock unchanged. The implementation lock records their identities and the new schema, validator and vector identities. This is an explicit interpretation/amendment outside the frozen packet, applicable to both designs. C01–C16 and the delivered platform §§8–9 override an abridged illustrative object.

A reusable AppDefinition and its publication contain input **schema contracts**, renderer/operator identities and declared commands. Concrete source revisions belong to a SavedWork binding. In particular, the §6.2 abridged example's `connection-events@rev1` is not permission to pin the author's evidence into every reused app. A new work selects compatible sources under its user's current authority (C02/C09/C10). Every persisted envelope carries server-derived organization scope; a client/model command supplies no principal, organization, policy grant, path, SQL or executable code.

Definition, source, publication, work, receipt and command/outcome identities remain distinct. A publication binds one admitted immutable definition digest, input/record-schema/operation/renderer identities, current policy and approval evidence. Work pins that publication and its source bindings; new publication does not reinterpret it. Adoption is an explicit compatible compare-and-set with audit; rollback never reverses records or restores access.

Structural schemas are necessary, not sufficient. The fixture harness checks declared references, schema/binding compatibility and same-scope relationships. Actual authorization is always performed against current server state, including the commit and result-delivery checks. Passing a synthetic fixture never establishes a principal's real access.

## Canonical bytes and identities

Use the explicitly restricted `forma-json-v1` encoding, **not** a claim of general JCS interoperability. Schema keys are ASCII; strings are valid Unicode scalar values, UTF-8 encoded without normalization. Reject duplicate keys, floats, non-finite numbers, unpaired surrogates and unsafe JSON integers. Sort object keys by ASCII order, preserve array order, omit insignificant whitespace and retain explicit null only where admitted. Canonicalization does not reorder operations or change meaning.

Identity is SHA-256 over UTF-8 domain, a zero byte, then canonical bytes. Domains name the contract kind and version, so a command payload and receipt with the same JSON do not share an identity. Exact decimal/money values are canonical decimal strings with explicit unit/currency and minor-unit exponent; binary floating point never determines business arithmetic. Business integer strings are bounded to 38 digits; schema counters, versions and coordinates remain safe JSON integers. The first synthetic money capability admits USD at minor-unit exponent 2 only, matching Spend Review; other currencies require a reviewed registry/edition extension rather than guessed currency semantics. This restriction is explicit and is not a claim of complete ISO 4217 support. The decoder enforces its byte bound before parsing; structural/semantic validation and canonicalization enforce the remaining bounds before a contract is accepted. This fixture harness is not a hostile-input resource sandbox. Timestamps in this edition use normalized UTC at whole-second precision; offsets and fractions require a future explicit contract extension. The implementation artifacts own the exact grammar, ranges and vectors consumed by later Rust/browser adapters; each adapter must reproduce those bytes before integration.

The first implementation edition establishes the compatibility baseline. Future revisions must retain prior valid vectors/fixtures, reject previously forbidden authority/execution fields, preserve tags, and explain any intentional API-major or semantic change. There is no prior executable implementation edition against which to invent a compatibility pass.

## Revocation: the five-second bound is total

Platform N3/N5/SI4 and demo SR6 require total display invalidation within **5,000 ms under healthy connectivity**. Platform §11.3/§6.5 and demo §5.2 prose describing a five-second poll plus request time cannot satisfy that total. For both designs, replace that scheduling assumption with this implementation budget: poll interval at most 2,000 ms, request deadline at most 2,000 ms, clearing allowance at most 1,000 ms. This is a budget to prove, not a measured promise.

Track freshness from the request start, not a late response's arrival; an overdue, stale-epoch or out-of-order response cannot extend the display's authority. Missing fresh authorization clears or masks protected content on expiry. On reconnection/resume, reauthorize before re-presenting protected results. Server requests, queued execution, commits and delivery independently check current scope; a browser poll never authorizes a request. Already delivered/exported information cannot be recalled.

APP-049/054 own runtime/browser timing and delayed/reordered/offline/resume evidence; APP-014 owns the commit ordering race, and APP-009/012 own request/delivery enforcement. Until those tests pass, this is an accepted target with open proof obligations.

## Retry, completion and concurrency

Synthetic outcome retention is 24 hours. The idempotency scope is organization + principal + work + command + key; keys require at least 128 bits from a cryptographically secure generator at the browser or pre-activation form renderer. A shape check cannot establish entropy. Current authorization precedes outcome disclosure. Within the transaction, the stored outcome is resolved before a new expected-version comparison: identical payload returns the prior result; different canonical payload under the same key conflicts. Current visibility is checked again before delivery. Expired outcomes require explicit reconciliation where duplicate effects matter, never automatic resubmission under a new key.

Command mutation, outcome and audit are one PostgreSQL transaction. Membership/work locks fence revocation/adoption, and record locking fences expected-version writes (platform §6 D5, §8; APP-013/014). No HTTP-entry-only authorization or cache stands in for that boundary.

An import, generation, export or indexing acknowledgement means **accepted/queued**, not committed business success. Record commands, publication, adoption and saves succeed only after the owning transaction commits; an object is durably written before a naming row commits. A lost response means unknown to the client, so it looks up the outcome with the same key. Failed and committed outcomes are distinct from transport uncertainty. The contract fixtures exercise these representations; later owning tickets prove persistence and replay.

## Developer and managed environment boundary

The integrated synthetic demo uses managed PostgreSQL/RDS, versioned S3 and organization key handling from the first integrated row (delivered demo D2/§8). One process has distinct workspace/control/data/worker interfaces and constrained role credentials (demo's typed in-process principal substitutes for platform SR3 service tokens). Local in-memory/disk doubles are permitted only for isolated unit tests and must be labeled. They cannot satisfy managed connectivity, isolation, durability, restore or integrated demo acceptance.

APP-007 owns the concrete environment manifest, migration connectivity, two-tenant checks, resource identities and cleanup plan. Account/access profile, region, spending cap and operator are still unresolved; those facts gate paid provisioning and managed-environment acceptance, while local contract/parser/query work proceeds. Default deployment is one ECS task; selecting the allowed VM alternative requires a recorded disposition. No account, resource, credentials or customer-data authorization is implied by this ADR.

## Reversibility and follow-through

These files version a new implementation boundary; frozen designs remain unchanged. Reject incompatible or unknown input instead of coercing it into a different meaning. Future changes create a new edition and migration/compatibility record. APP-003–007 consume these contracts during qualification; APP-010/012/013 integrate admitted definitions, receipts and atomic commands. No phase gate passes solely because schemas or their fixture tests pass.
