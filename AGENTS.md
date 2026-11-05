# Working in this repository

Read README.md, Tickets/README.md and the assigned ticket first. Use the forma-stack skill for implementation and the released package contracts documented in docs/architecture/FRAMEWORK-BASELINE.md.

Use [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for setup, commands, instance ownership, checks and delivery. `main` is the trunk and the beta deploy branch ([ADR-019](docs/architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)).

- This is the app platform, not the Forma framework or a donor canvas prototype. Own changes here; route framework/auth changes to their existing owners.
- Phase 1 implementation is authorized. Work through ready tickets and their reviews; the presence of later phases alone does not authorize their implementation.
- Ticket Markdown in status folders is authoritative. Keep one ticket file per APP ID; regenerate tracker, phase/sprint/category pages, coverage and index.json through Tickets/_tools/board.py.
- Read the dependencies and source references. Do not treat a planned checkbox, a design assumption or a framework release as proof that application behavior exists.
- Frozen design snapshots are evidence. Record new decisions in docs/architecture or docs/planning; do not silently rewrite design history. Later delivered design text outranks stale preparation prose.
- Use typed AppDefinition and trusted renderers. Model output is data, never executable code, SQL, markup, FMIR or grants. Both example apps must use the same general path without tenant-name branches.
- The target rendering mode is Rust/FMIR SSR plus explicitly registered canvas islands. Preserve the complete PageResponse security contract. Optional router adoption requires its own measured decision.
- Keep identity, tenant isolation, receipt provenance, idempotency and transactional audit intact even in a small synthetic demonstration.
- New dependency selections and commands belong in the owning implementation ticket. Pin actual released versions and lockfiles; never invent runtime commands in planning documents.
- Keep credentials, real source data, expanded build output and private runtime state out of Git. Preserve synthetic evidence needed to review the ticket.
- Use relevant checks, then record exact commands/results and limitations. Independent review covers authorization, concurrency, contract drift and meaningful failure paths.

Run `just check` and the focused campaigns appropriate to the change; `just check --list` and `just campaigns` name them. Regenerate the board after ticket edits, then run `python3 Tickets/_tools/board.py check`. Follow current user authorization; historical upstream protocol snapshots do not grant actions here.

Phase definitions and completion gates live in Tickets/plan.json. Read the assigned phase and docs/planning/QUALITY-GATES.md. Passing planning checks never passes application acceptance; mark a phase gate passed only with completed tickets, predecessor gates and independently reviewed runtime/trial evidence.

## Where to read

Read README.md, then docs/DEVELOPMENT.md, then the assigned ticket. Each major folder has its own AGENTS.md. docs/README.md classifies every document under docs/. Tickets/TRACKER.md is the board view. TRACKER, PHASES, SPRINTS, CATEGORIES, COVERAGE, BLOCKERS, the phase, sprint and category pages, and Tickets/index.json are generated. Read evidence/ and design/ from a path in a ticket or an ADR. Do not search them for current guidance.

## Hashed folders

`app/migrations/`, `parser/` and `contracts/` have no AGENTS.md on purpose. Every file in them is a release input: `app/migrations/` is hashed into the deployment contract digest that promotion compares with its baseline (`ops/promotion/candidate.py`), `parser/` into the parser source hash, and `contracts/` into `implementation-lock.json`. Adding any file there changes a release and stops automated beta promotion until a reviewed release. Migrations rules are in app/AGENTS.md. The parser is a separate sealed workspace reached through `imports` and `supervise`: build with `just parser`, probe with `just campaign parser-sandbox`, never fold it into the root workspace, and remember the images qualify their own parser. Contracts are the locked cross-language edition ([ADR-002](docs/architecture/ADR-002-CONTRACT-EDITION.md)): verify with `just check contracts`, and never change a published edition's fixtures or digests in place.
