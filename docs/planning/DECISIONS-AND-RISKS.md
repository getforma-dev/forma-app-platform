# Decisions and risks to retire

These are bounded decisions attached to implementation work, not reasons to stop preparation. Product/architecture choices already made are in [DECISIONS](../architecture/DECISIONS.md).

| Item | Responsible team | Resolution and timing |
|---|---|---|
| Native composition is not a named upstream conformance fixture yet | FormaStack; GenUI verifies consumer | FORMA-246 before native-canvas qualification. Require named upstream conformance, reviewed/merged evidence, exact consumable ref and GenUI rerun; no closure inferred from 3.2 publication |
| Design byte target differs from 3.2 and configurable app budgets are still planned | FormaStack + GenUI | Resolve FORMA-232's application-budget dependency before full native-canvas acceptance. Measure against shipped caps first. Configurable override work is needed only if the measured application requires it; no silent size-limit bypass |
| Current hosted fAuth revision/client readiness is unverified; historical deployment lag requires revalidation | FormaStack | FORMA-247 before integrated hosted login. Require current deployment identity, platform client, two tenants, redirect/PKCE and claim evidence |
| fAuth production signing-key custody | FormaStack | FORMA-248 before real organization sign-in/customer data. It does not stop synthetic fixtures, the parser or the local contract work |
| Native router may add too much eager code | GenUI | Start ordinary native page navigation. Decide optional native groups only after actual canvas bytes and retained-island behavior are measured |
| Five-second revocation target includes network/request time | GenUI | APP-002 records the total 5,000 ms disposition in [ADR-002](../architecture/ADR-002-CONTRACT-EDITION.md). APP-049/054 must measure scheduling, request, clearing and freshness behavior; new requests always reauthorize |
| Parser library can satisfy no-system-interface WASM and XLSX semantics | GenUI | Prove archive-wide bounds, fuel/memory and formula/link policy before implementing imports. Reject or reduce unsupported behavior explicitly |
| Cold million-row queries and six-view refresh may miss target | GenUI | Confined DuckDB spike before query-service commitment; measure admitted/rejected load and exact arithmetic; define supported shapes and honest limits |
| Demo and preparation documents describe different store choices | GenUI | Latest demo D2 controls: cloud PostgreSQL/S3 and tenant keys from row one. Local unit-test substitutes are not integrated deployment evidence |
| Account, region, budget, provider/model terms, retention and operator unknown | GenUI/product owner | Record before provisioning or real-data use as appropriate. Current preparation creates no resources or spend commitments |
| Demo can pass visually while product claim fails | GenUI | Keep genuine second-app authoring, unchanged build identity, current grants, independent reuse and no hidden developer intervention as hard exit checks |
| Restore/deletion and audit integrity can lag feature work | GenUI | Synthetic restore/deletion rehearsal before demo acceptance; hosted custody, anchors and deletion-ledger replay before pilot entry |
| No customer demand or price validated | GenUI/product owner | Pilot learning measures recurring use and effort. Do not turn synthetic completion into adoption or efficiency claims |

Deferred requests are recorded separately. Critical security invariants and exact calculation/transaction semantics cannot be traded for a presentation deadline.
