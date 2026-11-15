# APP-216 part 1: the catalog digest in PostgreSQL (2026-11-15)

By Claude (Opus 5.5), on branch `feat/app-216-readiness-digest` from `main` at `d00b9395` (schema 25). The change is [ADR-004 amendment A6](../../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md#amendment-a6-2026-11-15-the-catalog-comparison-compares-a-digest-the-database-computes-app-216-part-1): the database hashes its catalog document and the service compares 32 bytes against the digest of the pinned document in the same canonical text; any other answer runs the full comparison as before. No file in `app/migrations/` changed, and the deployment contract digest is unchanged.

## Measurement: bytes the database transmits per readiness run

`storage::qualification::readiness_traffic::native_readiness_traffic_campaign` (`just campaign readiness-traffic`, new, not run in CI) starts the APP-008 Docker fixture (PostgreSQL 16.15), builds `Stores`, runs readiness once to create the four connections, then reads the container's `eth0` counters (`/proc/net/dev`), runs readiness 20 times and reads them again. The same probe ran on the code before the change (the full comparison on every probe) and after it.

| | Before ([json](readiness-traffic-before-2026-11-15.json)) | After ([json](readiness-traffic-after-2026-11-15.json)) |
| --- | --- | --- |
| Catalog comparisons in 20 runs | 80 | 80 |
| … of which pulled the document | 80 | 0 |
| Catalog document (jsonb text) | 612,478 octets | 612,478 octets |
| Bytes transmitted by the database, per run | 2,507,967 | 32,305 |
| At the 15 s cadence | 167 KB/s, 14.4 GB/day | 2.2 KB/s, 186 MB/day |
| Bytes received by the database, per run | 91,814 | 89,430 |
| Wall time per run (local, unoptimized) | 198 ms | 206 ms |

Readiness still compares every pool on every run (80 comparisons in 20 runs); the comparisons stopped pulling the document. The wall time did not fall: the database still builds the document for the hash (the CPU part of the audit's cost), which the readiness interval, not this change, decides.

Limits: the counters are the container interface's, with TLS records and protocol framing included, for all four role pools; a local Docker PostgreSQL on a shared macOS host, not the beta's RDS instance. The beta's figure is its RDS `NetworkTransmitThroughput` metric after the release; the audit read 372 KB/s there, which this local probe does not reproduce exactly (connection creation, the helper's probes and the deployment's own traffic are not in the local number).

Commands:

```sh
GENUI_READINESS_TRAFFIC_REPORT=$PWD/evidence/APP-216/readiness-traffic-before-2026-11-15.json \
  cargo test --locked -p forma-app-platform --lib storage::qualification::readiness_traffic::native_readiness_traffic_campaign -- --ignored --exact --nocapture
# (catalog.rs changed)
GENUI_READINESS_TRAFFIC_REPORT=$PWD/evidence/APP-216/readiness-traffic-after-2026-11-15.json \
  cargo test --locked -p forma-app-platform --lib storage::qualification::readiness_traffic::native_readiness_traffic_campaign -- --ignored --exact --nocapture
```

Both: `test result: ok. 1 passed`.

## Tests

- `storage::catalog::tests::the_pinned_document_round_trips_through_its_canonical_text`: `parse(canonical(pin)) == pin`, and the expected digest is the SHA-256 of that text. This is the safety argument: a digest match means the live text equals this text, so the live value equals the pinned value.
- `storage::catalog::tests::canonical_text_is_jsonb_output`: the canonical text of a specimen with every JSON shape and every character `escape_json` treats specially (`"`, `\`, `/`, `\n`, `\r`, `\t`, backspace, form feed, U+0001, U+001F, U+007F, `é`, `€`, a CJK key), with jsonb's key order.
- `storage::qualification::native_postgres_campaign` (`just campaign postgres`), the "canonical catalog comparison" check, extended: a healthy readiness run pulls no document; each existing drift (`ALTER TABLE … DISABLE ROW LEVEL SECURITY`, `GRANT UPDATE ON genui.publications TO genui_data`, `ALTER TABLE … RENAME`) is refused after a digest mismatch (the full-comparison counter rose) and the restored catalog matches by digest again; the database's digest equals the SHA-256 of the body it would have sent and the service's expected digest; `canonical(parse(body)) == body` on the live document; and the specimen's canonical text equals the server's `jsonb` text of the same value. Result: `test result: ok. 1 passed` (61.9 s).

## Break-and-restore

Each break was applied to `app/src/storage/catalog.rs` by a scratch script, `just campaign postgres` ran, and the file was restored; its SHA-256 after each restore equalled the saved copy (`d0c95593985e629ef8c9117c4a1761f946c44a0b1ad714be7414cbd4914a665b`; `cargo fmt` then reformatted the file before the commit, which changes only whitespace).

| Break | Where the campaign failed |
| --- | --- |
| B1: the digest skips one catalog dimension on both sides (`body::jsonb - 'table_acl'` in the database, the pin without `table_acl` in the service) | `qualification.rs:922`: `GRANT UPDATE ON genui.publications TO genui_data` — the extra-grant drift was admitted (`assert!(empty.readiness().await.is_err())`). The row-security drift before it was still refused (another section). |
| B2: the service's canonical form diverges from the server's (object keys bytewise instead of shorter-first) | `qualification.rs:902`: `a healthy readiness run pulls no document: left 78, right 74` — the digest never matched, every comparison fell back to the full document, and the divergence was not silent. |

After the restores the campaign passed (the result above). The unit test `the_pinned_document_round_trips_through_its_canonical_text` also fails under B1 (the expected digest is no longer the digest of the pinned document's text).

## Not done here

- The readiness interval as a deployment setting (part 1's second step), the task size (part 2) and the park procedure (part 3): the ticket stays open for them.
- The beta's own `NetworkTransmitThroughput` reading: after the release that carries this change.
