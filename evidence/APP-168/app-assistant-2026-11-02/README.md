# App assistant and saved-answer iteration

Local-only qualification on `pivot/local-ui-iteration`, 2026-11-02. Preview: localhost:4001. No push, PR, merge, cloud deployment, database reset or pipeline monitoring.

## Result

- One X, Card/Browse/Open, symmetric gear; duplicate minimize/open controls removed.
- One app-owned companion with Ask, Summary, Sources and Saved. Library has Library/Commands/Shortcuts.
- Originals open inline with the existing paginated reader; a citation selects the exact file. Larger modal viewing remains available.
- Per-app Ask retains its session draft/thread and same-context question queue; source selection and collection scope remain exact. No builder or generic action fallback.
- Saved answers are private, app-owned snapshots. Save files them, Place on canvas is optional, X/Put away/Put all away preserve the saved object, Delete remains explicit. Canvas snapshots link back to the owning app. Reload keeps app answers filed instead of scattering them.
- Every source receipt remains inspectable through bounded pages. Record-answer Make it a card remains an explicit live-view preview.
- Server pinning verifies exact work/file lineage, immutable digest, canonical parsed sheet/row/label, current grants, and retry identity. No migration.

## Local checks

- `just check admin`: TypeScript, 239 tests, production SSR/island build passed. Canvas gzip JS 207,387 bytes under its 373,760-byte budget; Forma runtime 25,968 under 28,420.
- `just check test`: 386 Rust library tests passed, 34 opt-in campaigns ignored; 8 binary/SSR tests and 3 doctests passed.
- `just check http-smoke clippy-dev`: native HTTP/security smoke and all-target dev-feature clippy passed.
- `just check fmt test`: formatting plus Rust/SSR/doc verification repeated after final formatting.
- `just campaign box`: isolated final campaign passed in 105.73 seconds, including parser-backed saved references, forged labels/digests/non-lineage file refusal, privacy, idempotency, file-only grant revocation/restoration while app access remains, and no-record answer ownership.
- Independent review found an XLSX sheet/row provenance gap and an invalid-file-pin rendering fallback; both were fixed. Lifecycle, exact scope, denial, stale responses, queueing, receipt paging and durable-save/temporary-placement behavior have focused regressions.

The first combined checks encountered fixtures being updated during the iteration and a compiled-source snapshot changed by formatting. Final reruns above replace those intermediate failures.

## Actual browser checks

On the running local service, not mocked routes:

1. Library has no Ask tab; app header has one close control and no source disclosure.
2. Sources loads its exact original, opens five real spreadsheet rows inline with no modal or horizontal panel overflow; Open larger remains available.
3. Ask about file selects that file, obtains a real model answer, preserves source references and saves successfully through `/v1/cards`.
4. Save leaves zero answer placements. Saved lists it; Place on canvas shows the snapshot, original-file references and app link.
5. X removes placement; reopening Saved retains it. Put all away preserves the stored answer.
6. A second real answer with a source-row citation saves; its citation opens the correct original inside Sources.
7. Summary loads the existing finding through its attached view. Switching tabs does not generate another summary.
8. Close/reopen retains the unsent question and answered conversation. Closing a placed answer returns the camera to its existing app.
9. Reload restores the saved answers drawer inventory with zero automatic answer placements.
10. `/checking1 <question>` opens that exact saved app's assistant and produces a file Ask request only; Library remains closed and no builder/action POST occurs.
11. At 390×844, the companion moves below the app, the send button is reachable through its scrolling body, and there is no horizontal panel overflow. Desktop keeps a compact Card beside the taller companion.

Screenshots stayed outside Git because local original files may contain private data. Only behavior/metadata is recorded here. Browser test placements and the two test-created saved answers were removed, and the desktop viewport restored; original works/files remain intact.

## Limits

- Read-only Ask APIs answer each question independently. The visible session transcript is not provider conversational memory and is not persisted across reloads.
- Data writes use existing app actions/forms and their authorization/confirmation; natural-language autonomous data mutation was not introduced.
- File Ask's bounded model context is unchanged; original viewing uses the existing large-file paging limits. Generated prose is not recomputed when saving, while cited original rows and record receipts are checked.
- These ports use local persona sessions, not hosted sign-in. No hosted behavior or beta release is claimed.
