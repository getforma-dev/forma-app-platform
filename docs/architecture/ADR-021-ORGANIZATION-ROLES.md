# ADR-021 — Organization roles

Date: 2026-11-15. Owner: GenUI. Status: **accepted** (Victor's decisions of 2026-11-15, recorded by the coordinator; built as APP-225 and APP-226).

This ADR extends [ADR-007](ADR-007-PHASE-1-WRITE-AUTHORITY.md) (write authority, the admission chain, the definer pattern), [ADR-008](ADR-008-YC-DEMO-AND-BETA-MVP.md)'s 2026-11-14 amendment (the in-product admin area, D4) and the sharing roles of the 2026-11-14 amendment (viewer, contributor, reviewer on a work). It replaces the organization's one governance row (`organization_permissions.permission = 'admin_policy'`, ADR-007 D7.3) with a role per membership. Migration 0025 carries it.

## 1. Why

Forma is a SaaS platform for organizations. Every organization needs the same small set of professional roles: who runs the organization, who manages its members, who builds, who only looks. Until now the platform knew one organization-level fact, "holds `admin_policy`", written by the operator; everyone else was a member with whatever app grants they had. A class, a team or a company cannot run on that: there is no read-only seat, no way to delegate roster management without handing over everything, and no notion of an owner who cannot be locked out.

The roles are platform-level and tenant-agnostic. No class, cohort or job title appears anywhere: the platform never names a "teacher", a "TA" or a "student" (AGENTS.md: both example apps use the same general path without tenant-name branches).

## 2. The ladder

One role per active membership, `genui.memberships.role`, from the closed set `owner`, `admin`, `manager`, `member`, `viewer` (highest first). Guests (non-members on shares) are unchanged: they are not members and hold no role.

| Capability | owner | admin | manager | member | viewer |
| --- | --- | --- | --- | --- | --- |
| Build or create apps, use any AI (compose, Ask, Shape, folders, `/v1/act`, the customizer, MCP compose, Slack) | ✓ | ✓ | ✓ | ✓ | ✗ |
| Open and read apps and records they're allowed to see | ✓ | ✓ | ✓ | ✓ | ✓ |
| Write records or commands, upload sources, export | ✓ | ✓ | ✓ | ✓ | ✗ |
| Share apps (including organization-wide and guest links) | ✓ | ✓ | ✓ | ✓ | ✗ |
| Mint agent (MCP) tokens or connect Slack | ✓ | ✓ | ✓ | ✓ | ✗ |
| See `/admin` (members, roles, usage) | ✓ | ✓ | ✓ | ✗ | ✗ |
| Set a role to member↔viewer on a member or viewer, and remove members or viewers | ✓ | ✓ | ✓ | ✗ | ✗ |
| Set roles among manager, member and viewer on managers, members and viewers, and remove them | ✓ | ✓ | ✗ | ✗ | ✗ |
| Grant or revoke owner or admin, transfer ownership | ✓ | ✗ | ✗ | ✗ | ✗ |

In code the table is one function, `storage::OrgRole::can(Capability)` (`Build`, `Read`, `Write`, `Share`, `Connect`, `Admin`), and the management rows are `OrgRole::manages` (an owner manages everyone, an admin up to manager, a manager up to member), `may_set`, `may_remove` and `assignable`. Every route gate reads the role from the trusted request (`TrustedRequest::role`, resolved with the session, the Slack link or the agent token under the member fence) and refuses with the problem code `viewer_read_only` (403) before any resource grant is judged.

## 3. Two layers: the role is a ceiling

Effective access in an app is the lower of the organization role's capabilities and the share or resource role (ADR-007's grants: `discover`, `run`, `record_read`, `record_write`, `record_review`, `share_work`, `export`; the sharing roles viewer, contributor, reviewer). A viewer given an organization-wide contributor share reads the work but cannot write to it; a member given a viewer share reads only that work. Nothing in the role grants access to an app: `discover` and the work grants still decide what a member sees. The role decides what kind of actor they are.

The ceiling is enforced in the database as well as in Rust (APP-226, defense in depth): every SQL clause storage composes for a write, a review, a share, an export, a build or an upload carries `authorization::not_viewer` (`work_clause` for `record_write`, `reviewer_clause`, the `edit_definition`, `review_publish`, `share_work` and `export` branches of `permissions`), and migration 0025's RESTRICTIVE `viewer_read_only_{insert,update,delete}` policies refuse a viewer's member transaction on every write table (apps, definitions, publications, saved work, records, commands, shares and grants, sources and folders, agent tokens, Slack links). A transaction with no principal (admission, the operator, a guest) is not judged by them.

## 4. Helpers are not a role

A helper (an assistant, a shift lead, a reviewer of other people's entries) is a **member** plus the existing app-level `reviewer` share role on the apps they help with, plus **manager** or **admin** if they should also run the roster. The `/admin` page says so in one line. No "assistant" role exists and none will: the sharing roles already express per-app trust, and a second ladder would make the two drift.

## 5. The last owner

An organization never loses its last owner. Demoting or removing the last active owner is refused (`last_owner`), by the owner themselves and, trivially, by anyone else (an admin cannot touch an owner at all: `not_allowed`). An owner may demote or remove themselves only while another active owner remains. Organizations with no owner today (the operator-managed synthetic organizations) stay valid: the rule governs transitions, never the current state. The operator's `set-role` honors it too. The check is serialized: every owner transition of an organization (in `set_member_role`, `admin_remove_member` and the operator's `set-role`) first takes the transaction advisory lock `forma:owner-change:<org>`, so under READ COMMITTED two owners stepping down at once queue, and the second one sees the first one's commit and is refused; the organization is never left without an owner.

## 6. Who changes roles, and how

- **In the product**, on `/admin`: an owner, admin or manager sees the members with a Role column; each row offers a select with only the roles that caller may set on that member (`OrgRole::assignable`), a "Change role" button and, for an owner or admin change, a confirmation box. The server enforces the ladder regardless (`org_admin::set_role` in Rust, then `genui_private.set_member_role` under the member fence). Nobody raises their own role (`self`). A member or a viewer who reaches `/admin` gets the uniform 404.
- **From the operator channel**: `admission set-role --organization --principal --role`, audited on the operator chain; `grant-admin` and `revoke-admin` stay as the runbook's aliases for `set-role admin` and `set-role member`. `issue --role member|viewer` names the role a code admits with (default `member`; managers, admins and owners are promoted after joining, never by code). `list` shows each redeemer's role.
- **Never by a model, a share or a code template**: model output is data; `admin_policy` was never grantable by code and the role is not either.

## 7. The audit

Every real role change advances the target's `membership_epoch` (their open sessions re-read authority at their next request, as removal does) and appends `membership.role_changed`: on the admission chain for an admin's action, with the caller as actor; on the operator chain for the operator's `set-role`, with the target as actor (as the operator's revocations record it). Each carries an outcome digest recomputable from the target id, the role before and the role after under `forma:member-role:v1`. The same role again is a no-op: no write, no event. Names never enter the chain.

## 8. Migration 0025 and the data mapping

Additive but for one drop and one recreation, both recorded: `organization_permissions` is dropped (it never held another permission, and a second source of admin standing would drift from the role), and `org_admin_overview` is dropped and recreated because its `is_admin` column becomes `role` (a return type cannot change in place; the 0023 precedent). `admin_remove_member` is replaced in place under the ladder with `last_owner` for `last_admin`; `steward_card` and `admit_redemption` are replaced in place (the steward is an organization owner or admin; an admission takes the code's role).

Data, deterministic: in each organization the active `admin_policy` holder admitted earliest becomes `owner`, by the earliest `code_redemptions.committed_at` of the principal, NULLS LAST (a seeded or operator-admitted member has no redemption), then the smallest principal id; every other active holder becomes `admin`; everyone else, revoked memberships included, keeps the default `member`. FORCE RLS is lifted for that one statement and restored right after. One consequence to check after release: an operator-admitted admin has no redemption and so ranks after any code-redeemed admin; if the mapped owner is not the intended one, correct it with `set-role owner`.

## 9. Tiers and seats (later)

Roles are organization data. Tiers (A13; APP-222 part B) will gate them and count seats: a plan may cap the number of `member`-and-above seats, allow viewers at a different price or for free, and reserve `manager` for a higher tier. Those limits live in per-organization plan data, never in a constant sized for one customer (the 2026-11-15 rule), and the gate will be judged where the role is set (`set_member_role`, `set-role`, admission with a code's role), not where it is read. Nothing in this ADR constrains that; the role column, the audit kind and the capability function are what a tier will read.

## 10. Consequences

- `authorization::Action::AdminPolicy` is `Action::OrgAdmin`: the owner, admin and manager roles on the member's own active row; no permission row exists for it.
- The names and cards stewardship clause (`labels::OWNER_OR_ADMIN`, `steward_card`) names an organization owner or admin where it named an `admin_policy` holder; a manager runs the roster, not the content.
- The canvas hides Create app, Connect an agent, Share and the AI entry points from a viewer and shows one sentence in place of the composer bar; a viewer who reaches a route anyway gets `viewer_read_only`, with a themed page for a browser navigation (the APP-217 error layer).
- Campaign `org-roles` (`storage::org_roles_campaign`) proves the ladder cell by cell, the last owner, the epoch, the audit, the viewer walk in storage and over the routes, and `/admin` over real rows; `app/runtime/test_admission_operator.py` proves the data mapping (zero, one and several holders) and the operator's `set-role`.
