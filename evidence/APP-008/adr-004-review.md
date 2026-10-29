# ADR-004 independent decision review

Accepted direction review, 2026-10-29. No remaining blocking contradiction against the [seven planning findings](planning-review.md).

Reviewed [ADR-004](../../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md), SHA-256 **`9a7a74a5b0e893667f121f770cb2c72c99e272f946e1f4fad88065e98412fe88`**.

The final correction makes the order explicit: verified/server-mapped identity → BEGIN/install trusted transaction scope → current nonfinal precheck → scoped outcome when applicable → final authority fence. There is no unscoped membership lookup or universal membership lock at checkout.

The decision also resolves narrow NOLOGIN lock ownership and exact callers; organization/member writer fences; private non-cloneable transaction permits; exclusive pool/driver ownership and capacity/deadline bounds; a separately versioned private identity registry and noncircular admission seam; and the real schema/version2 delivery scope. Later session, entitlement, publication and command responsibilities remain with their owners.

This accepts an implementation direction only. It supplies no dependency/MSRV, migration, RLS, lease-cancellation, same-connection reuse, managed database or hosted authorization evidence. Those remain implementation and independent-review gates.

No implementation, dependency installation, cloud call or Git operation was performed for this review.

Follow-up, 2026-10-29: after APP-008 moved to In Progress, the sole ADR change is its ticket href from `Tickets/1-selected/APP-008.md` to `Tickets/3-in-review/APP-008.md`. Reversing only that href reproduces the exact previously reviewed hash; the new target exists. Current ADR SHA-256: **`0925434de4a123c013400aeb13b600cc136fd9a5a8003a96e526ef1faf58af60`**. Decision semantics and acceptance are unchanged; no full review was repeated.
