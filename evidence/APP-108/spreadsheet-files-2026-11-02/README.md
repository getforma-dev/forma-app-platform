# Original spreadsheet files on the canvas

PR 42 was reviewed against the clarified file-first interaction: uploading CSV or XLSX places a file icon. Double-click, Enter or Space opens the original file viewer. Creating a data card or an app is a separate, explicit action. Closing/removing a canvas placement does not remove the upload.

The original PR preserved uploaded bytes but called the inferred, first-sheet/20-row sample “the sheet as stored.” That was not a complete original-content viewer. The correction adds an authorized original-cell read route with 50-row pages and workbook-sheet selection. It preserves original lexical values, whitespace, leading zeroes, header cells and blank spreadsheet coordinates through the last stored cell. It never uses the normalized source, generated app, query result or inferred sample for these pages. CSV additionally has a complete raw-text view, including quoting and blank physical lines. Original downloads retain the exact uploaded bytes and digest.

## Qualified behavior and boundaries

- File references and positions persist in this browser under the server's member-scoped digest. No filenames or cell contents are written to local storage. Reload resolves current authorized metadata before showing an icon. Saving while metadata restoration is pending preserves those references. A file-only restored canvas is fitted only if the camera did not move during the read.
- Original reads use the existing member fence, entitlement and per-file folder/share authority. The new closed, bounded, same-origin POST checks authority before object IO and again after confined parsing. The original download now also rechecks after object IO. Responses are no-store. A forbidden sibling/tenant/anonymous/revoked reader receives the uniform 404. Already delivered document contents are not a live revocation lease; every subsequent read is reauthorized.
- Strings and filenames render as text nodes; formulas are never evaluated. Dragging away and back does not open the file; a single click selects it, double-click opens it, and keyboard removal does not also open it. Delayed page responses cannot overwrite a newer raw view or reopen a closed dialog.
- The immutable object store, existing sealed parser ABI/source/limits, schema 19, runtime configuration and deployment contract remain unchanged. No new parser, library, schema, grant or migration is introduced. The optional existing app-creation path still uses its explicitly separate inferred preview.

## Honest format limits

Uploads remain limited to 20 MiB. The table viewer uses the existing sealed parser's demo profile: UTF-8 CSV 10 MiB / 100,000 logical records; XLSX 5 MiB / 10,000 row coordinates per sheet / 16 sheets; 32 columns; 320,000 cells; 2,000 characters and 8 KiB per cell; bounded archive expansion and a 30-second per-parse deadline. A bound or safety refusal produces no partial table. Raw CSV and original downloads retain the full 20 MiB upload allowance.

XLSX viewing shows stored cell values, not Excel formatting, merged-cell layout, rendered date formats or formula calculations. Numeric/date serial values retain their lexical storage form; explicit empty coordinates remain blank. Formula-bearing, macro-enabled, encrypted, external-link and malformed workbooks are refused by the parser while their uploaded original remains available. XLS, XLSM, ODS and other spreadsheet formats are unsupported; they are not advertised in the picker. This is an original-value viewer, not an Excel editor or a promise to reproduce workbook formatting.

## Evidence

The [qualification report](checks.json) binds final source hashes and actual check results. [Browser checks](browser.json) use the owned 4701 loopback stand with synthetic member identity and actual upload/storage/parser/read routes; they make no model/app-creation request. The native box campaign uses a fresh TLS PostgreSQL fixture, real object store/parser and normal HTTP routes; its existing model-dependent cases use a labelled deterministic stub. These are local qualifications, not hosted/provider acceptance or deployment.

The initial full check exposed a stale SSR wording assertion from the original PR; it was updated to the actual “Bring your files” label. Browser testing exposed and fixed the async restoration camera gap. Earlier browser attempts also retained a stand-restart timing failure and an automation polling incompatibility with strict CSP; the harness now uses ordinary locator waits without weakening CSP. Private intermediate logs remain in this checkout's cache.

Screenshots show the [file icon](file-icon.png), [original table](original-cells.png), [raw CSV](original-csv.png), [second workbook sheet](original-workbook.png), and the [320px viewer](original-cells-320.png). The 320px check caught and fixed mobile padding extending beyond the modal width; page controls wrap and the close button stays reachable. These screenshots predate the merged PR40 theme/header changes. Existing global canvas chrome is outside this bounded mobile qualification.

Formal APP-108/APP-135 predecessor and broader acceptance gates stay open. No AWS resources, owner listeners, real-user files or credentials were changed by this review.

## Combined branch qualification

The combined source merges the reviewed PR40 theme/scan-context changes, PR41 drag affordance, PR44 command qualifications and the promotion cancellation record through normal merges. The [combined browser report](combined-browser.json) records another complete 11-check journey on that source. Final [320px file icons](combined-file-icon-320.png), [320px original viewer](combined-original-cells-320.png), [390px file icons](combined-file-icon-390.png) and [390px original viewer](combined-original-cells-390.png) show the updated header and file viewer together. The four captures were visually inspected; the icon and modal stay within their viewport, the header does not overlap, and paging/close remain reachable.

The [combined check record](combined-checks.json) binds the exact tested source and merged ancestors. The combined deployment contract equals the reviewed PR40 contract; this does not claim it equals the previously deployed contract or authorize a schema/pin bypass. There was no AWS deployment or provider action in this qualification.
