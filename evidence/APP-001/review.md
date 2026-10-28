# APP-001 independent review

Date: 2026-10-28. Reviewer: independent GenUI review agent (`app001_review`).

Reviewed implementation: `d220722` (Qualify published Forma 3.2 baseline and prepare upstream handoff), including source and lockfiles committed as ordinary files. Verdict: **no blocking findings** for APP-001's bounded qualification scope. CI and landing are separate outstanding closure conditions and must be recorded by the implementation lead.

## Inspection and acceptance

- The harness uses published exact Forma 3.2.0 pins. Archive verification covers the five npm packages and three Rust crates, compares the selected package checksums to active lockfiles, and verifies the known peeled release tag. The report distinguishes archive identity from a reproducible build of the upstream packages; optional router remains uninstalled.
- ADR-001 compares the old 3.0 assumptions with 3.2 rendering, native subset, state limits, ordinary and navigation-group budgets, and response-policy contracts. The application target is not silently raised from 28,100 B. Native full-library measurements/adoption remain APP-003/FORMA-232/246 work, and hosted-auth/custody remain APP-006/058 gates.
- All ordinary build/dev commands retain strict SSR. Required route-mode checks occur before typed binding, finalization and listening. The normal handlers return the entire PageResponse; SSR errors do not select a Browser shell. The retained starter Browser branches are unreachable with the mandatory startup guards.
- The executable HTTP probe checks initial Rust-rendered count/controls, a static no-JS page, no-store/nosniff, matching CSP/HTML nonces and freshness between responses. Its process is loopback-only, gets an assigned port and is terminated in a finally block. It explicitly does not claim browser adoption or platform functionality.
- Retained results identify clean npm installation, typecheck, strict build, locked Rust build/test and formatting. Earlier failed install/TLS/formatting attempts are disclosed with their resolutions. No frozen design snapshot or upstream runtime is changed.
- CI is configured to reproduce the fixture on Linux using the qualified Node/npm/Rust toolchain versions and committed lockfiles. The review does not substitute for the actual CI result.

## Checks performed during review

- `python3 Tickets/_tools/board.py check` — passed: 72 tickets, six phases, 12 sprints, 80 requirements; source digests and links valid.
- `git diff --check` — passed.
- Independently recomputed both active lockfile SHA-256 digests against `registry.json`; verified the three installed npm Forma package records are exactly 3.2.0, router is absent, and the portable npm lock includes the Linux x64 compiler binary — passed.
- Inspected source, test commands and retained outputs rather than repeating the already passing build. No new browser, managed-storage, auth, performance or deployment campaign ran in this review.

APP-001 may close once required CI succeeds and the reviewed change lands. No phase exit gate is passed by this review.
