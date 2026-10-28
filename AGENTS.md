# Working in this repository

Read README.md, Tickets/README.md and the assigned ticket first. Use the forma-stack skill for implementation and the released package contracts documented in docs/architecture/FRAMEWORK-BASELINE.md.

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

Current preparation checks: `python3 Tickets/_tools/board.py check`. Implementation will add focused application checks in APP-007 and subsequent tickets. Follow current user authorization; historical upstream protocol snapshots do not grant actions here.

Phase definitions and completion gates live in Tickets/plan.json. Read the assigned phase and docs/planning/QUALITY-GATES.md. Passing planning checks never passes application acceptance; mark a phase gate passed only with completed tickets, predecessor gates and independently reviewed runtime/trial evidence.
