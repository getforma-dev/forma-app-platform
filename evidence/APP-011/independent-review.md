# Independent APP-011 package and reset review

Reviewer: GenUI root, separate from the fixture implementation author. Reviewed the generator, independent CSV/OOXML reader and exact arithmetic, provenance manifest, parser/query adapter, setup manifest, disposable reset runner and focused guard tests against the frozen S01–S07 scenario obligations.

No blocking finding in the fixture package or reset implementation. Exact reviewed fixture bytes and independently executed checks are recorded in [independent-package/checks.json](independent-package/checks.json). The final dependency-bound reset below also passed. The three package acceptance items are supported; actual source CI/landing remain pending before ticket completion. This review does not close any authenticated application journey.

## Evidence and acceptance mapping

| APP-011 acceptance | Reviewed evidence |
| --- | --- |
| Actual original sources and provenance | Thirteen CSV/XLSX pairs and two deliberately malformed XLSX files. All 32 package files regenerate byte-for-byte. The manifest binds original byte lengths, SHA-256, sheet/columns and frozen source documents; traversal, symlinks, duplicates, changed bytes and unlisted files are refused. |
| Independent expected arithmetic | The verifier reads originals independently of generator constants. Exact integers/rational arithmetic reproduce Northstar 1,140,000/900,000/114,000, ops-runner 210,000/30,000/21,000 and USD Spend Review 64,000/45,000. Reviewed half-open UTC windows, exact-match/longest-prefix inventory precedence, missing coverage, ambiguous timestamps, units and currency controls. Fourteen focused tests pass, including values beyond JavaScript's exact-integer range and threshold equality. |
| Deterministic two-org setup/reset | Root independently ran the real disposable setup/mutation/reset and injected-failure cleanup. All 26 tenant tables, every row/column, two organizations, 13 principals and three explicitly seeded contract-shape bundles are compared. Four SQL roles prove own-org reads, foreign/missing/cleared scope denial, and denied owner escalation/direct writes. A committed operator mutation survives refused reseeding; destroying only the owned database and creating a fresh one reproduces the original logical state. |

The actual earlier [parser/query campaign](local-2/summary.json) exercised all 28 inputs and 14 supported typed queries across four datasets. Root checked every original input digest and qualified WASM digest against those retained reports. Later setup/reset metadata changes leave all original parser bytes unchanged; the historical source manifest is preserved rather than relabeled as current. Inventory/CIDR, rate/coverage and currency operators remain independent oracle checks where APP-005 has no corresponding runtime operator.

Root's first independent [successful reset](reset-independent-1/summary.json) and [deliberately failed cleanup trial](reset-independent-failure-1/summary.json) retain their original source pins and outcomes. The latter remains failed and confirms removal of its sole owned container. The [normalized-catalog follow-up](reset-independent-final/summary.json) passed with fresh source pins and both owned containers removed. That directory name predates the later temporary-schema/probe-order review and is retained as historical evidence.

After the final schema/probe repair, root repeated the [dependency-bound reset](reset-independent-catalog-path-final/summary.json): both workspaces reproduced logical digest `sha256:cf22271640f77ae5c8b540e6406f5c813064886713a46c87cfb35b41695af9f2`, both owned containers were removed, and every recorded dependency source pin still matched afterward. The changed digest includes the reviewed normalized catalog resource; the earlier state digests remain intact. This final run is bound to runtime `dfef99b5f60fe225b480563a8b54977cfdc45f99887b9a05a9904b4a3f918beb` and query `e17ceb5e62dc6f03dfb9dc26f34018d3f2977e1b93ee5910ed16f03364a63963`.

## Authority and limits

The CLI accepts a new evidence directory only. It refuses ambient PostgreSQL or Docker target overrides and non-local Docker contexts. The helper creates uniquely named loopback-only containers; cleanup checks identity before removing one and records observed absence. There is no target URL, existing-container or cloud-profile selector. Failure records are retained, and SIGTERM/SIGINT take the cleanup path; SIGKILL or host loss requires operator reconciliation.

The isolated operator mutation uses organization → member → work locking and is labeled as a test mutation. Shape seeds retain placeholder admissions, normalized sources, records and receipts from APP-002. Neither the mutation nor a passing reset represents a user command, actual import, authenticated session, provider account, publication or backup recovery. APP-057 owns shared/authored rehearsal reset. SQL role/RLS checks do not replace native principal authorization.

The new fixture workflow builds the real locked parser, retains the existing qualified WASM digest check, exercises the original package/query subset and runs disposable PostgreSQL setup/reset. Its actual CI result and final source commit must be recorded separately; a workflow definition is not a passing run.
