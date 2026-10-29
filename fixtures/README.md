# Synthetic acceptance inputs

APP-011 packages actual original CSV and XLSX bytes under [phase-one/v1](phase-one/v1/manifest.json). Its authority is the delivered [S01–S07 scenarios](../design/forma-northstar-demo/SCENARIOS.md), [C05/C06/C16 contracts](../design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md) and frozen APP-002 implementation lock. Every input records its original SHA-256, byte length, columns, worksheet, source section and any invented control rows. No customer data or credentials are included.

The package contains 13 equivalent CSV/XLSX pairs plus two malformed XLSX controls. Events and inventory have separate original files. The ops-runner case has its own source and reserved work identity. Canary inputs deliberately contain distinct values (40 total bytes), so a cross-organization cache mix-up cannot look correct merely because fixture values match. All XLSX cells are literal values except the explicitly refused formula specimen; ZIP entry order, timestamp, permissions and uncompressed storage are fixed for byte reproducibility across compressor versions.

| Fixture | Required expectation |
| --- | --- |
| Northstar | 1,140,000 total bytes; 240,000 matched; 900,000 unmatched; 114,000 bytes/minute |
| Independent ops-runner | 210,000 total; 30,000 unmatched; 21,000 bytes/minute |
| Spend Review | 64,000 integer USD cents; `>= 20,000` queue contains only EXP-002, 45,000 cents |
| Boundary controls | Lower timestamp included, final whole second included, upper timestamp and wrong asset excluded; 50 total bytes |
| Overlapping inventory | Exact match wins; otherwise longest IPv4 network; event counted once |
| Invalid mapping | Invalid, ambiguous and fractional timestamps refused; undeclared units, cumulative counts and mixed currency refused |
| Partial coverage | Observed total retained; full-window rate and unobserved zero buckets remain unknown |
| Malformed XLSX | Formula/cache and excessive dimension refused by bounded parser |

The selected window is `[2026-10-22T10:00:00Z, 2026-10-22T10:10:00Z)`. The rate denominator is the full ten minutes. “Unmatched” means absent from the supplied inventory, with ownership and activity unverified. [mappings.json](phase-one/v1/mappings.json) contains fixture-authored interpretation; this does not claim a user confirmed an application mapping. [expected.json](phase-one/v1/expected.json) records design expectations, controls, and the **unexecuted** future record-command sequence. No committed Spend Review decision is seeded into these source files.

## Reproduce and verify

From the repository root:

```sh
python3 fixtures/generate.py --check
python3 -m unittest discover -s fixtures -v
python3 fixtures/verify.py
spikes/APP-005/.venv/bin/python fixtures/qualify.py --output evidence/APP-011/new-run-name
```

`generate.py --output /an/empty/directory` generates a separate byte-identical package. Existing nonempty directories are refused; generation never resets application state. Verification reads the original CSV/OOXML files independently of generator row constants, then uses exact Python integers, rational rates and an independent IP-network oracle. Tampered bytes, unlisted paths, duplicate manifest entries, in-package symlinks, ambiguous inventory and malformed numerical values fail. The manifest also binds the frozen source documents.

The qualification requires the existing locked APP-004 parser host/WASM and APP-005 Python environment described in their READMEs. It runs every input through the actual WASM parser, compares original values and coordinates to the independent reader, and retains raw reports. It binds the parser to APP-004's recorded qualification inputs rather than to its recorded bytes: every source file, `Cargo.lock` and vendored ZIP provenance hash in [build-metadata.json](../evidence/APP-004/build-metadata.json) must match byte-for-byte, `verify_vendor.py` must pass, the rustc/cargo release and commit must be the recorded 1.93.1, ambient Cargo/rustc build overrides are refused, and `cargo build --locked` is rerun so the artifacts cannot be stale. The resulting WASM digest is recorded in `summary.json` and bound into every parser report; it is not required to equal the recorded arm64 macOS digest, because the wasm32 build embeds absolute Cargo registry and checkout paths and APP-004 claims no cross-host byte identity ([reproducibility.json](../evidence/APP-004/reproducibility.json)). A parsed result remains `inspection_required`, including inputs later refused by interpretation checks.

Four event datasets then feed APP-005's real typed query API through an explicitly disclosed trusted fixture adapter: it selects the declared asset, maps event IDs to temporary integers and writes temporary Parquet. This is a **synthetic spike adapter**, not the app's future import pipeline. Query results/receipts cover sums, destination groups, observed minute buckets, a selected-destination filter and pagination. The spike supports only its events shape; it does not implement the inventory/CIDR join, coverage/rate operators or Spend Review input. Those expectations are oracle checks only. This local run does not repeat APP-005's operating-system isolation/performance campaign.

Each qualification output directory must be new. Raw failures and earlier runs remain available; evidence must not be overwritten to make a failed gate disappear. See [APP-011 evidence](../evidence/APP-011/README.md).

## Two organizations, setup and reset

[setup.json](phase-one/v1/setup.json) names the actual managed synthetic organization IDs ending `0001` and `0002`, distinct scoped principals, exact fixture-only issuer/tenant/subject tuples, expected epochs and APP-008 seed interfaces. Northstar contract object IDs intentionally repeat inside both organizations to detect missing tenant keys. The same UUID string in another organization is a separate principal/object; provider identity requires the full issuer + provider tenant + subject tuple.

APP-008's `app/runtime/tenant_seed.py` creates three revalidated **contract-shape bundles**: Northstar in both organizations and a canary-only Spend Review for foreign-key negatives. Their source/normalized digests, admission, records and receipts remain placeholders from the frozen APP-002 specimens. The fixture package's real original digests must not be substituted into these envelopes without actual normalization and admission. Its operator seed refuses changed existing rows rather than resetting epochs or granting broader authority.

The [disposable reset trial](reset_trial.py) now exercises actual local database setup/reset using APP-008's canonical migration and seed interfaces. It accepts only a new evidence-output directory. It cannot select a database URL, profile, image or existing container; ambient PostgreSQL/Docker endpoint overrides and remote Docker contexts are refused. It creates fresh, uniquely named local PostgreSQL containers with random loopback ports, verifies their identities, and removes only those owned containers.

```sh
PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python fixtures/reset_trial.py --output evidence/APP-011/a-new-reset-run
```

The trial establishes two organizations, 13 scoped principals and the three validated contract-shape bundles. It retains every row and column from all 26 tenant tables, migration versions/checksums and the canonical schema digest. Installation timestamps are retained separately because new installations have different times. Every SQL role exercises own-organization positive reads and foreign/missing-scope negatives before and after reset; this proves database roles/RLS, not provider login or native principal authorization.

Inside the first owned database, an explicit operator mutation locks organization, member and work in that order, advances one saved-work version and changes its context. A repeat seed must refuse to overwrite the change and leave it intact. The trial then removes that database, creates a second fresh database, applies the same canonical migrations/seed and compares the complete logical state. Both containers are removed in `finally`. New evidence directories preserve failures; SIGTERM/SIGINT take the cleanup path, while SIGKILL cannot run language-level cleanup. A separate retained injected-failure trial verifies cleanup after a committed mutation.

This is **disposable fixture-workspace reset**, not backup restore or a destructive API for shared state. APP-057 still owns later authored/shared managed rehearsal reset. Reserved Jordan/ops-runner identities are explicitly **unseeded future authored state**. Neither the seeded admissions/receipts nor the operator mutation is an actual application import, publication or record-command outcome.

## What this does and does not establish

| Scenario | Package evidence | Remaining application acceptance |
| --- | --- | --- |
| S01 | Original inputs, actual disposable two-org setup and exact seed-manifest alignment | Real login/invites, workspace navigation, accessible upload, meaningful creation and layout persistence |
| S02 | Source bytes, parser coordinates, exact oracle, supported event query subset | Confirmed mapping/import, registered inventory/rate operators, linked views and authoritative app receipts |
| S03 | Seeded shape provenance clearly labeled | Actual draft/preview, change/undo, independent review, publication and failure recovery |
| S04 | Independent ops source and expected arithmetic | User-created separate work, real grants/share/revocation/export/adoption and persistence |
| S05 | Exact USD source/threshold oracle; future command expectations | Same-path app creation, Finance access, real record commands/replay/conflicts/audit |
| S06 | Malformed parser inputs, invalid interpretation, integrity controls and owned disposable reset/cleanup | Runtime race/denial, queued revocation, source lifecycle, shared rehearsal reset and isolated backup restore trials |
| S07 | Byte pins, reproducibility and retained scoped check reports | Complete build/model/input/definition pins, actual receipts/outcomes and authenticated rehearsal log |

A fixture or seeded definition cannot stand in for user creation, publication, actual import, tenant authorization or the two-app rehearsal. APP-011's package checks do not close those gates.
