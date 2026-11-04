# Large spreadsheet viewing

Local iteration decision, 2026-11-02, at Victor's request. This extends original-file viewing; it is not a hosted deployment qualification. Earlier APP-108 evidence describing the viewer's 32-column demo profile remains historical.

## Separate viewing from governed import

An uploaded file remains an immutable original. Viewing uses a dedicated, read-only `view` parser profile. It never registers data, generates an app, changes a cell, evaluates a formula or makes a network request from the parser. Typed import and normalization retain their existing contracts and limits. In particular, wider viewing does not silently discard columns to satisfy the 32-field app contract. Ask and Make an app explain their narrower current support when inference refuses a file.

The existing sealed WASM parser scans and validates the input, but emits only the requested 50-row × 32-column window and global dimensions. The HTTP handler checks authorization before reading and after parsing, validates every output coordinate against the requested window, and returns a no-store response. Rows and columns can be paged or reached directly using row numbers and Excel column letters. Sparse cells and ragged CSV records retain their positions. Raw CSV text is also displayed in bounded pages. Downloads retain the exact original bytes.

## Admission and resource limits

| Boundary | Viewing limit |
| --- | --- |
| Uploaded file | 20 MiB; existing upload limit |
| Expanded XLSX contents | 100 MiB total, at most 512 ZIP entries |
| Workbook sheets | 16 |
| XLSX shared strings | 100,000 |
| Grid coordinates | 1,048,576 rows × 16,384 columns |
| Scanned cells | 4,000,000 across the file/workbook |
| Cell text | 65,536 Unicode characters and 256 KiB UTF-8 |
| Page response from parser | 8 MiB, at most 1,600 cells |
| View parser memory | 512 MiB guest; 768 MiB process RSS backstop |
| Processing | Existing 5 billion fuel ceiling; application deadline 30 seconds per parse |
| Concurrent original reads | Two per server; excess requests receive 429 with Retry-After |

These are admission ceilings, not a promise that every file at every maximum can be processed. CPU, memory or expansion limits may reject a smaller pathological file. Refusal leaves the stored original intact, with a specific explanation and download access. The preview does not quietly return a partial file as a complete result.

## Excel compatibility and safety

The viewer admits ordinary stored/deflated XLSX ZIP entries, including streamed ZIP data descriptors after their CRC, sizes, local headers and complete nonoverlapping spans have been checked against the central directory. Strict import keeps its earlier ZIP subset. Unsupported compression, encryption, ZIP64/extra-field aliases, malformed/ambiguous archives, macros, external links and DTD/entity declarations remain refused.

Formula expressions never execute. Existing saved formula results are shown and marked; the viewer states that they may be stale. A formula with no saved result is visibly marked **No saved result**, not represented as an ordinary empty cell or a fabricated zero. Numeric/date serials retain their lexical storage value. Styles, merged-cell layout and recalculation are outside this original-value viewer.

## Verification

Qualification uses synthetic wide and tall CSV/XLSX files, first/middle/final pages, ragged records, long Unicode text, cached/missing formulas and hostile archives. The reported user files are inspected only from local stored originals; their contents are not committed as fixtures or sent to a model. Measured results and exact commands belong in APP-108's iteration log.
