# APP-138 — Forma Canvas brand and interface

October 31, 2026. Local implementation; no AWS/OVH deployment.

## Source and review

Base: `818ca9dfda875b99e2a1f52c909e3147765b32bf`. Reviewed application diff SHA-256: `5445f916bdb7f9400909ea652215ff55bbbeb2d029f4b7261daac1f4fd6c15f3` (`git diff --cached --binary -- app .dockerignore` before implementation commit). [Design direction](../../docs/architecture/FORMA-BRAND.md).

Independent reviewer: existing developer_guide agent, which did not implement this change. Read-only source review of that exact identity: **no blocking findings**. Reviewed native binding and output policy, unchanged action hooks/forms/receipts/authority, responsive CSS, Docker font allowlist, same-origin font delivery and retained OFL. Reviewer did not independently rerun builds or browser checks; results below were measured by the implementer.

The reviewer identified an inherited `pointer-events: none` on the welcome scroll surface. Reproduced at 390×667: wheel input over the heading left scrollTop at 0. Fixed with pointer events on the welcome surface and contained overscroll; rechecked scrollTop 151.5 with all choices reachable.

## Checks

- `just check`: all 18 steps pass (format, clippy, dev clippy, strict admin build/typecheck, Rust tests, MSRV, catalog, infra, Python suites, operations, schema pins, contracts, board and shell syntax).
- `npm --prefix app/admin test`: 48 pass.
- Final public-entry removal followed by `npm --prefix app/admin run typecheck`, `npm --prefix app/admin run build` and `just check fmt clippy clippy-dev test py-qualification board`: all pass. Rust: 345 library, 7 binary SSR, 3 doctests; 30 intentionally ignored campaigns. Qualification: 10 tests, including tracked source image allowlists.
- New home regression verifies the empty native props contract, successful no-store response, invitation/sign-in entry links and no scripts/islands. Initial test-harness failures came from omitting the required bind/finalize sequence; the harness was corrected without changing production behavior.
- Production canvas JavaScript: 157,661 bytes gzip against 373,760-byte ceiling. Home and About have no JavaScript. The self-hosted font loads under existing CSP; no remote font request or new CSP permission.
- `git diff --check` and regenerated ticket board pass.

## Browser observations

Chromium/Playwright, local pitch and isolated browser contexts; existing stored app data preserved. Layout changes from testing were confined to the automation browser's local storage.

- 1440×1000 light/dark welcome: real three-way start controls, Forma wordmark/product tile, shared Space Grotesk and paper/charcoal/lime palette. Font load confirmed via FontFaceSet.
- Library: app names remain readable on their own row; search, expand, add individual cards and close remain operable. The original single flex row had reduced the first app title to zero width; the revised row gives it 205px at desktop and 156px at 390px width. Bottom-sheet width and content width both 390px, with no horizontal overflow.
- Populated canvas: claims total, review queue and category chart added through library, fitted to viewport; real data/verified receipts render in both themes. Receipt inspector opens and closes; zoom menu and fit work. Canvas cards and minimap use the shared palette.
- 390×844 and 390×667: choices stack, header/actions and command bar remain in view; short-screen welcome scroll verified. Reduced-motion emulation preserves the existing global animation/transition suppression.
- Native invitation/sign-in pages inspected in browser. Public home inspected using its actual Rust-rendered test response exported temporarily to an ignored preview folder and served over loopback: 1440×1000 light and 390×844 dark, no scripts, font loaded, no horizontal overflow, visible 2px keyboard focus. The temporary test export was removed before final source verification.
- Server rebuilds temporarily interrupted the development event stream; it reconnects after restart. A pre-existing optional lineage lookup for a seeded app returned 404. Neither is a font/CSP or presentation failure.

## Limits and rollback

No live-provider sign-in, real upload, model generation or write/approval command was exercised for this presentation-only change. Their existing action attributes and handlers are preserved and covered by existing tests. No data migration, cloud call, authorization change or framework update. Revert APP-138 source to restore the former presentation.
