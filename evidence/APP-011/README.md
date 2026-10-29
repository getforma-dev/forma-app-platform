# APP-011 source-package evidence

This evidence covers deterministic original inputs, an independent arithmetic oracle, actual bounded parsing, the existing event-query subset, seed-contract alignment and an owned disposable local database reset. It does not qualify an authenticated application journey, shared managed reset or backup restore.

The clean `local-2` run passed: nine focused tests, all 28 parser cases (26 accepted for inspection and two deliberate refusals), 14 actual typed queries across four datasets, and three revalidated seed bundles. Its source pins matched the working files at that initial handoff. [source-identity.json](source-identity.json) preserves the 37-file candidate from that point. Later reset implementation changes have separate source identity/evidence; the historical qualification files have not been rewritten. Integration/review remains owned by the lead.

| Evidence | Meaning |
| --- | --- |
| [initial-failure.md](initial-failure.md) | Retained macOS temporary-path verifier failure and incorrect formula-refusal expectation, with corrections |
| [local-1/summary.json](local-1/summary.json) | Failed first full run; the actual parser correctly rejected a formula, but its refusal code differed from the fixture's expectation |
| [local-1/parser](local-1/parser) | Original first-run reports, including both real malformed-XLSX refusals; never rewritten |
| [query-probe/summary.json](query-probe/summary.json) | Passed intermediate query/seed probe using first-run parser output; explicitly not a replacement for the final run |
| [local-2/summary.json](local-2/summary.json) | Clean rerun outcome, timestamps, source and executable hashes, counts and limitations |
| [local-2/unit-tests.log](local-2/unit-tests.log) | Focused fixture integrity, semantic boundary and byte-reproducibility checks |
| [local-2/oracle.json](local-2/oracle.json) | Independently calculated Northstar, ops-runner, canary, boundary, overlap, coverage, timestamp/unit and currency outcomes |
| [local-2/parser](local-2/parser) | Exact parser configurations and raw reports binding input/config/WASM hashes, budgets, cells/coordinates, mapping state or refusal |
| [local-2/query-subset.json](local-2/query-subset.json) | Actual typed queries and synthetic-preview spike receipts, temporary Parquet hashes and explicit source-to-spike mapping |
| [local-2/seed-alignment.json](local-2/seed-alignment.json) | Three APP-002 bundles revalidated against the two-org setup manifest; no database actions |
| [reset-local-1/summary.json](reset-local-1/summary.json) | Passed actual owned disposable setup → mutation → destruction → fresh setup → full logical-state comparison; both containers removed |
| [reset-local-1/workspace-1-initial.json](reset-local-1/workspace-1-initial.json) | Complete initial tenant-table rows, exact row counts and separately retained installation times |
| [reset-local-1/workspace-1-mutated.json](reset-local-1/workspace-1-mutated.json) | Real committed operator change; only first organization's saved work differs |
| [reset-local-1/workspace-2-initial.json](reset-local-1/workspace-2-initial.json) | Independently created fresh database has exactly the original logical state |
| [reset-injected-failure-1/summary.json](reset-injected-failure-1/summary.json) | Deliberate failure after committed mutation remains failed; its owned container was removed |
| [reset-source-identity.json](reset-source-identity.json) | Current reset candidate pins and explicit reuse of unchanged original-file parser evidence |
| [reset-validation.json](reset-validation.json) | Fourteen focused tests, byte-exact regeneration and independent oracle rerun for the reset candidate |

Canonical command, from repository root:

```sh
spikes/APP-005/.venv/bin/python fixtures/qualify.py --output evidence/APP-011/local-2
```

That directory is retained; reproduction must use a new output name. The package independently regenerates byte-for-byte, including all XLSX ZIP metadata. Execution reports contain measured timestamps/durations and therefore are not expected to be byte-identical.

The existing APP-004 qualified WASM digest is checked before this run uses it. Parser success means `inspection_required`; intentionally invalid timestamps/currency are still ordinary literal cells at this stage. Mapping refusals belong to the independent interpretation oracle. Query checks cover the registered event shape only, with four original datasets and 14 queries. Inventory/CIDR, rate/coverage and Spend Review arithmetic remain oracle expectations because APP-005 has no corresponding qualified runtime operators. There is no new OS confinement or performance claim.

The APP-008 owner confirmed that its operator seed validates before writing and refuses changed existing rows/epochs; it has **no destructive reset API**. APP-011 uses that unchanged interface solely inside fresh databases owned by its new [reset trial](../../fixtures/reset_trial.py). All 26 tenant tables, two organizations, 13 principals and three contract-shape bundles are compared before/after reset; all four SQL roles exercise both organizations and canary negatives. The full logical-state digest is `sha256:52246a09a6a7633cec8402c751b41dc91d1e36ee92409e4ccf44a6694cbca275`. The intentional failure campaign proves cleanup without converting its failed result into a pass.

Run the local disposable trial with a **new** output directory:

```sh
PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python fixtures/reset_trial.py --output evidence/APP-011/a-new-reset-run
```

Only `setup.json`/its manifest and generation/report metadata changed to describe this integration; all 28 original CSV/XLSX inputs retain the digests exercised by the earlier real parser campaign. That unaffected campaign was not rerun. The five new reset-target/evidence guard tests bring the local fixture suite to 14 tests. The setup manifest still marks admissions, sources, normalized hashes, records and receipts as seeded placeholders. APP-057 shared managed/authored rehearsal reset remains pending, and database role/RLS checks do not substitute for native subject authorization. This is neither actual source import nor backup recovery.

Root owns independent review, CI, source integration and ticket disposition. Passing this package does not close S01–S07, Phase 1 or the later two-app rehearsal gate.

The later [independent review](independent-review.md) supports all three package acceptance items. Root repeated generation, all fourteen tests and original-parser digest binding, then reran the disposable reset after the final catalog/probe repairs. [Final dependency-bound evidence](reset-independent-catalog-path-final/summary.json) records matching source pins, two removed containers and logical state digest `sha256:cf22271640f77ae5c8b540e6406f5c813064886713a46c87cfb35b41695af9f2`. Earlier receipts and their state digests remain historical. Actual source CI and landing are pending; APP-011 is In review.

Source `677d7bd` was pushed for CI. [The account-level refusal](../APP-008/ci/677d7bd-billing-block.json) prevented all ten jobs from starting. APP-011 is now Blocked on GitHub Actions billing availability, successful exact-source checks and main landing; its local acceptance/review evidence is unchanged. No failed test is inferred from those unstarted jobs.
