# Editable data in apps built from spreadsheets

Local iteration, November 2, 2026. This extends the [app assistant](CANVAS-WINDOWS.md). It is application code on the released Forma rendering/runtime baseline, not a framework change or hosted release.

## Two distinct datasets

An uploaded CSV/XLSX remains an immutable source. Its original bytes, normalized revision, digest and file viewer do not change when someone edits an app. Each new app built from supported spreadsheet data gets its own complete typed record collection initialized from the confirmed normalized data. Two apps built from one file own independent records.

The original input contracts and source bindings remain as lineage. Additional records inputs reuse the same field contracts; generated query references point to those records inputs. View IDs, component layout, presentation, app identity and work identity are retained. Add and Update are ordinary declared commands under those schemas, not model-created permissions.

Existing source-only apps can use **App assistant → Ask → Change data → Enable app data**. The review lists the files and complete row counts before confirmation. Adoption installs the new publication, collections, initial records, lineage and import manifest in one authorized transaction. It is pinned to the owner, publication and work version; retries do not append another copy. The canvas refreshes capabilities within the existing app window.

## Reading and changing data

Ask defaults to **App data** when the app has record collections, including an empty collection. **Original · filename** is an explicit alternate context. Sources opens the full original viewer and offers Ask about original. Historical saved answers remain snapshots.

In Change data, the member selects Add or Update; Update also requires an explicit existing record. The assistant can propose values only for the selected app's declared action. The review runs ordinary command validation without saving. Confirmation submits that exact request through the existing permission, optimistic version, idempotency and audit checks. Building and theming stay with the builder.

App query views run through the existing typed query compiler over a full authorized record snapshot. Exact decimal coefficients are retained; displayed row limits do not limit sums or counts. Receipts retain the original source revisions and identify the record snapshot used. Release rechecks authorization, publication, bindings and record versions. Summary regenerates or resolves its references using these governed results. Ask retains its separately disclosed bounded record context; it must not describe a partial sample as a full total.

## Resource boundaries

Viewing originals retains the [spreadsheet viewer limits](SPREADSHEET-VIEWING.md). Editable copies have a separate ceiling: 100,000 rows and 64 MiB of canonical typed record values per app, at most the existing 32 imported fields per table, and the record contract's 2,000-character text bound. The import transport also has a 140,000,000-byte bound including encoded record values and row identities; its overhead can reject a many-row copy before the 64 MiB value ceiling. Both budgets are checked before a new app is published. Unsupported or oversized copies are refused as a whole, never silently shortened. The original remains available. One import runs at a time per service process; a concurrent request receives a retryable 429. Add and Update enforce the current row and value-byte ceilings before saving.

Full query snapshots are also bounded before record bytes leave storage. They use private per-run files, a concurrency ceiling and the existing confined engine deadline/memory controls. Source originals are never rewritten to make app updates visible. No model call or background migration is triggered just by opening an app.

2026-11-14 (APP-200, Fable advisor in lieu of Victor, from the APP-197 measurement): the snapshot ceiling is **24** full snapshots per service process, `storage::record_query::SNAPSHOT_SLOTS`, derived from the query worker's running set plus one organization's admitted queue (`TASK_RUNNING` 4 + `QUEUE` 20), so one organization's views are refused first by the worker's frozen admission (D7.10), never by this cap; the slot past the ceiling answers 429 with `Retry-After` without waiting. Worst case: 24 × the 64 MiB per-snapshot bound, about 1.5 GiB in the 8 GiB task. The data pool still bounds database reads (one connection per capture, released with the transaction).

Local qualification and remaining limits are recorded in the owning APP-168 evidence and ticket. Beta publication remains outside this iteration's authorization.
