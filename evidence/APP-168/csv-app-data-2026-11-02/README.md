# Editable CSV app data — local qualification

Scope: Victor's local-only follow-up on `pivot/local-ui-iteration`. No push, pull request, merge, cloud migration or deployment is authorized by this evidence.

## Implemented behavior

- New spreadsheet apps initialize complete, independent typed record collections and declared Add/Update actions. Their existing typed views read those records; original uploaded files and source revisions remain unchanged.
- Existing source-only apps offer an explicit **Enable app data** review and confirmation. Adoption retains the app/work identity, original inputs, views and window; initialization and publication switch are transactional and replay-safe.
- The app assistant defaults to app data after adoption, supports explicit original-file questions, and saves proposed data changes only after confirmation through the existing audited command path.
- Update can search the full authorized collection, select an explicit record, review the proposed values and confirm. Numeric searches use displayed decimal/money values.
- Imports and queries are bounded without silently truncating editable data. See [design and limits](../../../docs/architecture/CSV-APP-DATA.md).

## Review

Independent review covered authorization, exact work/source bindings, immutable originals, transactional initialization/adoption, concurrent update fences, replay, bounded memory/concurrency, and stale member/guest query releases. Findings repaired during the loop: exact model-selected source matching, domain-typed PostgreSQL parameter casts, transport-size preflight, older-record numeric search and automatic Ask context reset. Schema migration 0020 was kept immutable after its first application to local preview 4001.

## Qualification

- `just check admin`: 271 tests, TypeScript and production build pass.
- `just check fmt clippy clippy-dev test http-smoke`: pass on the final source. 397 library tests, 8 SSR tests and 3 doctests; 34 opt-in campaigns remain ignored by the ordinary test runner.
- The remaining fast-check groups pass: MSRV, catalog, infrastructure, Python scripts/runtime/shared broker/qualification/app scripts/operations/promotion/infrastructure, schema pins, contracts and shell syntax. The runtime suite passed 198 tests. Contracts initially found DuckDB missing from its local venv; syncing the existing hashed locks restored the pinned dependency and the contracts rerun passed. No dependency versions changed.
- `just campaign box`: passes with a complete 2,505-row import, exact decimal aggregates after Add/Update, independent app copies, explicit older-record search, no-write proposals, duplicate replay, competing-update conflict, original download equality, legacy adoption and member/guest snapshot release fences. It also exercises bounded snapshot concurrency, collection isolation and source-grant revocation. Early failures found PostgreSQL domain parameter casts; two test assertions were corrected to the established receipt nesting and HTTP 409 record-conflict contract.
- `just campaign actions`: passes, including scoped record search and exact decimal/money SQL display checks. Independent PostgreSQL review additionally compared 3,268 signed/subunit/38-digit cases across scales 0–18 with the exact string formatter.
- Schema qualifier: passes against isolated PostgreSQL 16.13; runtime/catalog/manifest pins all agree on version 20. Packaging allowlist checks pass; release images were not built.

Actual browser on port 4001, using the real model and a synthetic three-row CSV:

1. Build from the uploaded file created an app with Add and Update actions automatically. Ask selected **App data**, with the original available as a separate context.
2. Add proposed `Delta`, November 2, USD 7.25. Storage still contained three records before confirmation. Cancel kept three records; a second review and confirmation created exactly the fourth record.
3. Find records located `Beta`; the explicit selection displayed USD 20.00. Update reviewed USD 20.00 → USD 22.50. The stored value stayed USD 20.00 until confirmation.
4. Open showed four records and USD 45.25 in both typed query totals and the records/ledger components. Sources → View original opened the larger modal with the unchanged original three data rows (USD 35.50), including Beta at 20.00 and no Delta.
5. A synthetic legacy source-only work was created through the existing one-card API to exercise adoption. Enable app data reviewed all three original rows. Confirmation installed Add/Update without replacing the window node or changing its identity, position, dimensions or Card view. Ask then defaulted to App data; that independent copy still contained the original three values.
6. Restart verification preserved the account's live membership epoch and saved data. A normal sign-out had exposed an existing local bootstrap bug: strict initial seed replay rejected the advanced epoch. The local dev launcher now runs migrations on every start and seeds only an uninitialized volume, using its durable epoch marker. Independent review confirmed that missing initialized database state fails closed and session minting reads current authority. Production/operator seed admission is unchanged.

The browser fixture folder is `Local CSV editing check`; the primary app work is `062dd791-037a-43df-b941-c3c61c0bd3ac`, and the legacy adoption fixture is `e3e62543-98ae-48b0-8fa7-c1210196ebb1`. Only synthetic rows were changed. Private runtime logs and real customer data are excluded from Git. The primary test app remains available for local review; auxiliary test placements are removed without deleting their saved works.

## Practical boundaries

Editable copies currently support 100,000 rows / 64 MiB of typed values with an additional transport-envelope bound, 32 imported fields per table and 2,000-character text fields. Oversized copies are refused completely; original viewing has its separate larger limits. Ask has its existing disclosed bounded context, while typed query totals use the complete authorized snapshot. No hosted sign-in, release-image build or AWS deployment is qualified by this local trial.
