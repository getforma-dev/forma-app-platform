# Focused evidence audit

Read-only audit on 2026-10-22 of PRODUCT-BRIEF.md, ALIGNMENT.md and SHARED-CONTRACTS.md. No repository files changed, tests run, or release archives downloaded.

## Actionable refinements

1. In ALIGNMENT’s Writes row, the earlier PDF already includes notes, findings, status, ownership and review records (p11). General-purpose forms and explicit creation/update of defined records are the new broadening. Avoid implying that all writable internal state is newly introduced.
2. In ALIGNMENT’s Review row, the earlier PDF already defines organization publication policy and overlapping roles (pp5,13). Mark this as retained/clarified policy rather than a new change. Northstar’s independent review remains a demo choice.
3. Use current renamed historical source paths in new citations. The old packet can remain unchanged. All renamed files match their prior hashes.
4. Preserve both current working-file hashes and release blob hashes for framework claims; four inspected current docs differ from their release blobs. The existing release pin is correct.

## Confirmed new evidence

- All 46 previous manifest files match their recorded SHA-256 hashes. The claim about 30 framework/prototype files is exactly correct. All eight recorded Git HEADs are unchanged.
- The local v3.0.0 tag resolves to 2006423b21fe0f8dd5a510bd09bfe3b8748327a5. The canonical HEAD is 753be2ae7e3838cd37035e5d892bba448e5793bc.
- Both primary PDF copies exactly match their originals, and fresh pdftotext -layout output matches the supplied review text. App-platform has 25 pages; earlier platform has 23. Both say Version 1.0 / October 22, 2026.
- Controlling-PDF page references in ALIGNMENT are accurate. Page 18 explicitly proposes a spending-table review queue with detail view/internal decision form as its different second app; pp20–21 require two different apps without per-example platform changes.
- The pinned release SECURITY.md states trusted application-build FMIR only and deferred untrusted-module admission. frame.rs permits only ready/resize/error replies; HTML-RUNTIME.md says trustedHtml is not sanitization.
- Vector and Generative UI manifests pin core 2.0.1 and pre-3.0 tooling/server; extracted runtime peer is ^2.0.0. Fieldwork pins 3.0.0 and its actual source uses typed native pages, startup bind/finalize, bounded JSON APIs, and a single-process counter store.
- Builder Stripe, GitHub, and Notion integration service sources each explicitly say mock implementation. These newly hashed source records support the existing caveat.
- Historical platform v4 says nothing of the platform is prototyped; insurance v5 explicitly says nothing of B is built. Security STATUS is pre-design preparation. No contradiction found in the new packet’s treatment.

## Handoff

source-manifest.json contains current absolute paths, bytes, SHA-256, prior-path mapping, prior comparison, release blob hashes where applicable, and claim support. prior-manifest-recheck.json is the raw comparison. Do not use either as new runtime qualification.
