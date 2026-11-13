# APP-135 slice 3: independent review (2026-11-12)

Scope: commit `8d92783` on `app135/folder-ask-disclosure`: `app/src/boxes/ask.rs`, the new test in `app/src/boxes/tests.rs` and the slice 3 ticket text. I did not write it. I read the code and ran unit tests only: no browser, no AWS, no dev stand, no campaigns. Line numbers are at `8d92783`.

Checks:

- `cargo test --locked -p forma-app-platform --lib boxes::` gave 41 passed, 0 failed, 1 ignored (`real_haiku_folder_smoke`, which needs a real key). That includes `boxes::tests::folder_answers_name_what_they_did_not_read`.
- `cargo fmt -p forma-app-platform --check` reported nothing.

## What holds

- **Files past the cap are not read.** The cap check and its `continue` (`ask.rs:499-503`) come before `routes::inspect_file` (`ask.rs:504`).
- **The totals add up.** Each candidate lands in exactly one of three places: the tables read (`out`), `unreadable` (`:507`) or `beyond_cap` (`:501`). So `total` (`:559`) equals the number of CSV/XLSX candidates (`:493`). PDFs and other non-table files are never counted. The sort (`:495`) only changes which files are read, never the total. Candidates come from the member's authorized snapshot (`storage/boxes.rs:320`, `file_visible()`), so the count reveals no hidden file.
- **`reader_missing` cannot be true while some tables were read.** `state.parser` is a fixed `Option`, set once at startup (`main.rs:258-262`; on the dev stand, `dev/mod.rs:353-372`). `inspect_file` returns `parser_unavailable` only when that option is `None` (`routes.rs:428`), before any I/O, and no other code produces that string. Either every read fails this way or none does. Besides, `coverage_note` never reads `reader_missing`; only the empty-tables path does (`ask.rs:676`, `:683`).
- **No release image shows `no_reader`.** The image sets `GENUI_PARSER_MANIFEST_FILE` (`app/Dockerfile:85`, asserted by `app/runtime/qualify_image.py:38`), and startup stops if verification fails (`main.rs:67-71`). Only a dev stand without `just parser` reaches it. The stand already prints that next step at startup (`dev/mod.rs:362`). The sentence correctly blames the server rather than the files. Its "right now" is acceptable.
- **The `{text, references}` shape is unchanged** (`document()`, `ask.rs:172-191`). `talk::judge` runs before the note is added (`:720-725`), so the note is never judged as model output. The canvas renders folder answers as text nodes (`admin/src/canvas/ask.ts:383`). The box campaign checks the keys and uses `contains` on the text (`storage/box_campaign.rs:1112-1115`, `:1765-1770`, `:1876-1904`), so a trailing note cannot break it. MCP and Slack never call folder Ask.

## Blocker

None.

## Major

**M1 (existing before this commit, but it limits the "Fixed" claim): the model can see no rows of a table, and nothing tells it so.**

- In `user()`, the first row that would overflow `MAX_MODEL_BYTES` marks only its own table `partial` and then leaves the loop entirely (`break 'rows`, `ask.rs:237-239`).
- Every later table is still sent, with `"rows": []` and its original `partial` flag (`:218-219`), which is `false` for a table of 200 rows or fewer.
- The prompt says only that a `partial` table was cut (`SYSTEM`, `:200`).

So when table 1 or 2 is wide enough to use up the 60 KB budget (for example 16 columns of about 15 characters over 200 rows, roughly 50 KB), the model gets later tables with no rows and no partial flag. Its answer then covers part of the folder without saying so, and the new note adds "This answer uses 3 of the 3 tables" or no note at all. This is the failure acceptance 2 forbids, through another door.

Fix: when the loop breaks, mark the current table and every later one `partial` (a one-line loop). Or record the case as a register row instead of calling folder partial answers "Fixed". This affects model answers only; rehearsed answers do not go through `user()`.

## Minor

**m1. An answer can now exceed the documented 1500 characters.** The module documentation promises "plain text (at most 1500 characters)" (`ask.rs:20`). `judge` accepts up to `MAX_TEXT` = 1500 (`composer/ask.rs:41`, `:356`), and the note then adds up to about 110 characters (with at most 20 files per folder, `storage/boxes.rs:37`). That gives about 1611.

Nothing breaks today. Folder-wide answers go only to `folderAnswer`, which has no cap (`admin/src/canvas/box-api.ts:404-418`), and they cannot be saved. The consumers that enforce 1500 only ever receive single-file answers:

- the app-chat file answer (`admin/src/canvas/app-chat.ts:46`);
- the answer card body (`composer/cards.rs:279`).

A single-file Ask never carries a note: its one file is either read or not.

It becomes a trap if folder answers are ever saved as cards. Either correct the documentation (1500 plus the coverage note), or judge model text against `MAX_TEXT` minus the longest note.

**m2. Two comments are out of date.** The documentation of `tables()` still says "The second value counts the data files that could not be read" (`ask.rs:483-485`). The module notes (`ask.rs:14-17`) do not mention the coverage note.

**m3. The note's wording.**

- "uses" overstates what happened: the model was shown N tables, and with M1 possibly no rows of some of them. "This answer reads N of the M tables" is more exact.
- The note is also added to the rehearsed refusal "Ask only reads, so nothing changed…" (`ask.rs:356`), which used no table at all. This is harmless, but it reads oddly.

**m4. `no_reader` speaks about a folder even when one file was asked about.** The file card (`admin/src/canvas/file-card.ts:52`) and an app's Ask on a file source (`admin/src/canvas/app-chat.ts:274-275`) send a `file_id`. Both now get "The files in this folder are kept; Brief me still reads its PDFs."

Separately, Brief me is open only to the folder's owner (`boxes/brief.rs:176`), so a member who can only read the folder is sent to a route that answers 404. `no_table` already has the same wording. This only matters on a dev stand.

**m5. The logs do not show a partial answer.** `box.asked` with code `ok` (`ask.rs:658-671`) logs `tables` but not `unreadable` or `beyond_cap`. Adding the two counts would make partial answers visible in operation.

**m6. The tests.** The new unit test covers only the pure sentence builders. The counting loop in `tables()` and the two places in `ask_post` that add the note (`ask.rs:696-701`, `:722-725`) are checked by reading alone. That is acceptable for this slice, because the loop is small and reads correctly, but two cheap tests are within reach:

- **A campaign case.** The box campaign's "Untitled folder" already holds a readable `Ventas ano.csv` and a newer, unreadable `empty.csv` (`storage/box_campaign.rs:1655-1675`). A scripted `folder_ask` on it should end with "(This answer uses 1 of the 2 tables in this folder: 1 could not be read.)".
- **A unit test of `tables()` with no parser.** The test `state()` already has `parser: None` (`boxes/tests.rs:27`), and `inspect_file` fails before it touches the store (`routes.rs:428`). Such a test could check `reader_missing` and the `unreadable` count without a database.

## Verdict

Approve with changes. The diff is correct:

- the counting does not mislead;
- files past the cap are not read;
- `reader_missing` is all or nothing;
- the response shape and the campaigns' expectations hold.

Before the ticket calls folder partial answers fixed, fix M1 or record it in the register. m1 and m2 are documentation fixes. m3 to m6 are optional.
