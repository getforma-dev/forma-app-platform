# APP-135 files, folders and naming

One fresh clean customer. No application code changed. Acceptance stays open. Sharing, MCP, Slack and the founder keep/remove/defer list were not exercised.

## Environment

| Item | Value |
| --- | --- |
| Commit | `af63feba41aa825a972dde354e0a5d2195f7d608` |
| Instance | Worktree `forma-pitch-rehearsal`, `just dev clean reset -d`, port base 7500, Alex at `http://127.0.0.1:7510/canvas` |
| Left alone | 127.0.0.1:4001–4003 and :4010 were not reset |
| Data | `reading.csv`: title, author, "The Dispossessed" and "The Left Hand of Darkness", both Ursula K. Le Guin. `empty.csv` is 0 bytes. `broken.csv` is an unclosed quote. `notes.txt` is two lines of prose |
| Browser | Playwright Chromium, 1440×900. Live model for **Build the app** (`source: model` on the apps response) |
| Demo | `data-demo` empty. Header "Synthetic organization 0002" |

Screenshots: [app-135-files/](app-135-files/).

## Steps

### Empty folder — pass

- Action: **+ Folder**, type `Reading notes`, Enter.
- Expected: one empty folder with that name.
- Observed: `POST /v1/boxes/.../name` 200, name "Reading notes", `file_count` 0, number 1. "Renamed to Reading notes." Ask / Make a card / Make an app stay hidden until a file exists; only **Brief me** showed. [02-empty-folder.png](app-135-files/02-empty-folder.png).

### Upload the CSV — pass

- Action: Add `reading.csv` through the folder's file input.
- Expected: the file is stored and a column preview opens.
- Observed: `POST .../files` 201, kind csv, 92 bytes. Preview: columns `title` and `author`, both text, 2 rows, sample "The Dispossessed" / "Ursula K. Le Guin". The sheet says "2 rows · 2 columns · types inferred by the parser; change any before you confirm."

### Preview and confirmation — pass

- Action: **Cancel** on the sheet. Open it again. **Make an app from this**, then **Build the app**.
- Expected: Cancel writes nothing. Build asks before it generates.
- Observed: After Cancel, the file still offered **Make data** and had no Data check. The second sheet's **Make an app from this** revealed **Build the app**. That click registered the data (`POST .../data` 201) and `POST .../apps` 201, origin `generated`, source `model`, label "Built from folder: Reading notes · 1 file".
- Cards: **Reading Notes Overview**, chip **Generated**, "Built from folder: Reading notes · 1 file". **Total Books** count 2, **Verified** `496a09a9`. **All Books** lists both titles and the author. **Books by Author** shows Ursula K. Le Guin, book count 2, **Verified**. [15-built.png](app-135-files/15-built.png).

### Rename — pass

- Action: click the folder title, type `Club notes`, Enter.
- Observed: `POST .../name` 200, name "Club notes". "Renamed to Club notes." After reload the provenance line reads "Built from folder: Club notes · 1 file". [16-reload.png](app-135-files/16-reload.png).

### Addresses — pass for the controls used

- Action: `/1` opens folder number 1. On the app card, **Save as /** was filled with `reading-list` and **Save** clicked. Then `/reading-list`, then a same-browser reload.
- Observed: `/1` opened Reading notes / Club notes. After Save and reload the page still showed `/reading-list` and Reading Notes Overview. `/rename` was not the control on the card; this walk did not type it.

### Reload — pass in the same browser

- Observed: Club notes, the generated chip, Total Books 2, both book rows, and the Verified line were still there. **Books by Author** was still **loading** in that frame. A new browser's local layout was not part of this check.

### Empty file — pass

- Action: add `empty.csv`.
- Observed: `POST .../files` 422 `unsupported_file`. "empty.csv: That file type is not supported here, or its contents do not match its name. Nothing was stored." The folder listing did not gain the file. No second folder or app.

### Malformed CSV — partial

- Action: add `broken.csv` (unclosed quote), then `notes.txt`.
- Expected: a file the parser cannot read as a table is refused, with nothing stored.
- Observed: `broken.csv` returned 201 and "broken.csv added." The sheet included "may be empty". **Make data** remained. No second app was created. `notes.txt` was stored and added. The folder then held `reading.csv` (Data check, 2 rows), `broken.csv`, and `notes.txt`. The built app stayed **Reading Notes Overview**. [18-broken.png](app-135-files/18-broken.png).
- Severity: low. The bad file is visible and did not duplicate the app. It does leave a file the member may try to build from.
- Suggested fix: if the parser cannot produce columns, answer as the empty file does (`unsupported_file`, nothing stored) and say why.

## Not a product failure

The first upload screenshot was taken while the progress bar still said "No files yet." The file landed, and the preview followed. That frame is not the outcome.

## Not tested

A second member seeing the renamed folder, MCP, Slack, and `/rename` as a typed command.
