# Module map: the modular monolith's layers and dependency rules

Status: the map of [ADR-013](ADR-013-REPOSITORY-AND-DELIVERY.md) D13.4 (Accepted, Victor, 2026-10-31: "Accept, approve all"). It **records**:

- what the product is: one binary, one process, four logical roles with distinct database grants (DECISIONS.md APP-D05);
- where each module sits;
- which `use crate::` directions are allowed;
- the edges that break the rules today.

**Enforcement is deferred to APP-097** (ADR-013, "Deferred"). Nothing here fails a build. `just check` has no dependency step yet; the APP-097 work is a `just check deps` allowlist built from this map.

The measured table below comes from `scripts/dev/deps.py`. It is report-only and always exits 0. It walks the release sources of `app/src` and collects every `crate::<module>` each module names. Out of scope, by D13.4 rule 3:

- `cfg(test)` modules, including those declared inside a test-only file, and the inline test module at a file's tail;
- `#![cfg(...)]` files, such as `auth_qualification`;
- the `dev-stand` code (`app/src/dev/`, `bin/genui-dev.rs`).

It was measured at `pivot/ladder` after the root workspace (APP-130), with the dev binary already split out of `storage`.

## The layers

A module may name modules in a **lower** layer. Layer 2 modules may also name each other; the cycles this allows are listed below. Layer 4 may name anything.

| Layer | Modules | What each is | May depend on |
| --- | --- | --- | --- |
| 0 Foundation | `config` | process settings and credentials file | nothing in the crate |
| | `roles` | the four role contexts (APP-D05) | |
| | `readiness` | composed readiness | |
| | `supervise` | confined child processes (ADR-009 D9.1–D9.2) | |
| | `origins` | the channel a record version came through | |
| | `directory` | member display names | |
| | `demo` | demo shortcuts scoped to synthetic organizations | |
| | `http` | the shared HTTP contract: problem+json, limits, the proxy (request/response helpers only) | |
| 1 Trust | `storage` | the only module holding a Postgres connection | layer 0 |
| | `auth` | sign-in, sessions, cookies, CSRF | |
| | `admission` | whole-output admission of typed definitions | |
| 2 Domain | `query` | bounded typed query with receipts | layers 0–1, and each other |
| | `composer` | describe an app, get a saved work | |
| | `actions` | the action path: utterance, typed draft, confirm | |
| | `commands` | the declared record command | |
| | `boxes` | folders: files, data, apps built from them | |
| | `sharing` | share a work; the members list | |
| | `widgets` | canvas utility widgets | |
| | `imports` | the sealed-parser import boundary | |
| | `events` | the in-process event hub and SSE | |
| 3 Surfaces | `pages` | server-rendered pages | layers 0–2, never each other (`slack` → `mcp` is the one kept exception) |
| | `mcp` | Forma as an MCP server; agent tokens | |
| | `slack` | slash command, buttons, approver DMs | |
| | the canvas (`app/admin`, TypeScript) | the canvas client; it talks to the server over HTTP only | |
| 4 Composition | `product`, `main.rs`, `bin/genui-dev` (ADR-012 D12.1) | the route composition and entry points | anything |

The rules (D13.4):

1. Nothing below layer 4 names `pages`, `product`, `main` or a `bin/`.
2. Each capability has one owner:
   - only `storage` speaks SQL (`tokio-postgres`, `deadpool`);
   - only `auth` verifies a session or a token;
   - only `admission` admits a definition.
3. `cfg(test)` and `feature = "dev-stand"` code is exempt. The campaigns reach everywhere by design.
4. The known leaks are recorded as debt and not fixed now (below).

Outside the module tree:

- `parser/` is a separate Cargo workspace and process (ADR-009 D9.1), reached only through `imports` and `supervise`.
- `spikes/APP-006` (`genui-auth-verifier`) is a workspace member library that only `auth` uses.
- `app/runtime/*.py` (the confined DuckDB engine, the box normalizer) are child processes, reached through `supervise`.

## Measured dependencies (release code)

Run `python3 scripts/dev/deps.py` to regenerate this table, or add `--json` for JSON output.

| Module | Layer | Names |
| --- | --- | --- |
| `roles` | 0 | `config` |
| `readiness` | 0 | `config` `storage` |
| `demo` | 0 | `storage` |
| `directory` | 0 | `storage` |
| `http` | 0 | `admission` `boxes` `composer` |
| `storage` | 1 | `actions` `admission` `commands` `composer` `config` `directory` `query` |
| `auth` | 1 | `http` `storage` |
| `imports` | 2 | `supervise` |
| `commands` | 2 | `admission` `auth` `events` `http` `storage` |
| `events` | 2 | `auth` `http` `sharing` `storage` |
| `sharing` | 2 | `admission` `auth` `composer` `events` `http` `storage` |
| `widgets` | 2 | `admission` `auth` `composer` `demo` `http` `storage` |
| `actions` | 2 | `admission` `auth` `boxes` `commands` `composer` `demo` `directory` `events` `http` `origins` `query` `storage` |
| `boxes` | 2 | `actions` `admission` `auth` `composer` `demo` `http` `imports` `query` `sharing` `storage` `supervise` |
| `composer` | 2 | `actions` `admission` `auth` `demo` `directory` `events` `http` `query` `sharing` `storage` `widgets` |
| `query` | 2 | `actions` `admission` `auth` `boxes` `commands` `composer` `demo` `directory` `events` `http` `storage` `supervise` |
| `pages` | 3 | `auth` `commands` |
| `mcp` | 3 | `actions` `admission` `auth` `boxes` `commands` `composer` `demo` `directory` `events` `http` `query` `sharing` `storage` |
| `slack` | 3 | `actions` `auth` `boxes` `commands` `demo` `directory` `events` `http` `origins` `sharing` `storage` |
| `product` | 4 | `actions` `auth` `boxes` `commands` `composer` `events` `mcp` `query` `sharing` `slack` `widgets` |

Rule 1 holds: no module below layer 4 names `pages`, `product`, `main` or a binary. The kept exception is `slack` → `mcp`, but in release code `slack` does not name `mcp` today, so the exception is unused.

**Layer-2 cycles (allowed for the MVP, listed):**

- `actions` ↔ `composer`
- `actions` ↔ `boxes`
- `actions` ↔ `query`
- `boxes` ↔ `query`
- `composer` ↔ `query`
- `composer` ↔ `sharing`
- `composer` ↔ `widgets`
- `events` ↔ `sharing`

They are the pitch's domain, grown together in one pivot. A crate split would freeze them into public APIs, which D13.3 rejects.

## Edges outside the rules

### Known leaks (named by ADR-013 D13.4 rule 4), recorded as debt

| Edge | Where | Status | Owner |
| --- | --- | --- | --- |
| `storage` → `pages`, `product`, `readiness` | was `storage/dev_stand.rs`, `pool.rs` | **Gone.** The dev binary moved to `app/src/dev/` (ADR-012 step 4, APP-128), as D13.4 predicted. `pool.rs` now names only `config`. | — |
| `storage` → `actions`, `commands`, `composer`, `query` (layer 1 → 2) | `storage/rules.rs` (`actions`), `storage/actions.rs` and `storage/commands.rs` (`commands`), `storage/labels.rs` (`composer`), `storage/queries.rs` (`query`) | **Open.** These are domain types that storage persists (a rule's action, a command payload, a label shape, a query plan). The fix moves the persisted shapes down into `storage` or a layer-0 type module, and the domain modules depend on them. | APP-097 (ADR-013 "Deferred": fixing the `storage` and `http` leaks) |
| `http` → `boxes`, `composer` (layer 0 → 2) | `http/mod.rs:221` (`boxes::routes::long_path` for the request budget), `http/mod.rs:232` (`composer::COMPOSE_DEADLINE`) | **Open.** These are route helpers reaching into domain types. The fix has the domain modules register their long-path budgets with `http`. | APP-097 |

### Found by the measurement, not named in D13.4

These follow from how D13.4's table reads ("nothing in the crate" for layer 0, "layer 0" for layer 1), not from new coupling. They are listed so that the APP-097 allowlist starts from the truth, not from the table. No decision is taken here. Amending D13.4 is APP-097's call.

| Edge | Reading |
| --- | --- |
| `http` → `admission` (layer 0 → 1) | `http/mod.rs:89` encodes a problem body with `admission::encode` (canonical JSON). A third `http` leak, of the same kind as the two named ones. Same fix: move the canonical encoder down to layer 0. |
| `demo`, `directory`, `readiness` → `storage` (layer 0 → 1) | These three read through storage: demo switches per organization, display names, and the database probe of readiness. They are layer-0 by role, not by dependency. The proposal for APP-097 is to move them to layer 2 (`demo`, `directory`) and layer 4 (`readiness`, which composes probes), or to split each one's pure part from its storage read. |
| `roles` → `config`, `readiness` → `config` (inside layer 0) | Intra-layer and acyclic. The proposal for APP-097 is to allow acyclic edges inside layers 0 and 1. |
| `auth` → `storage`, `storage` → `admission` (inside layer 1) | These are required by design: sessions live in `storage` (only `storage` speaks SQL, rule 2), and storage admits definitions through `admission` in the same transaction. They are acyclic (`storage` does not name `auth`), so they fall under the same proposal. |

## When it is enforced (APP-097)

`just check deps`: `scripts/dev/deps.py --json` compared with an allowlist file generated from the tables above. A new edge fails the check, and a removed edge shrinks the allowlist. It is the 40-line walk D13.4 describes (cargo-modules would be a new tool under AGENTS.md:13 for the same answer). Until then, run the report when a change adds a `use crate::` across modules, and update this map in the same commit if the table changes.
