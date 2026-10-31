# Canvas rules: what is on a canvas, how you talk to it, how things are named

This is the single reference for the canvas model and its grammar. The decisions are recorded in APP-120 (model and grammar), APP-121 (naming), APP-122 (Ask and Shape), APP-123 (cards made by you, `@` mentions, `/forma`, `/<folder>`, pinned answers), APP-124 (start from your data) and APP-119 (commands versus apps). Where this page and the code disagree, the code is wrong: file a ticket.

## The one sentence

**Type `/` and a verb to change the canvas, `/` and a name to tell that thing something, or just type a sentence.**

Four jobs, and where you type picks the job, never the AI:

| Job | What it does | Typed as |
| --- | --- | --- |
| **Ask** | Finds out: answers questions about your data. Changes nothing | `/ask [app or app.card] <question>`, or `? <question>`; `@app` or `@app.card` anywhere scopes it; a folder's **Ask** asks about its files |
| **Shape** | Changes how something looks or what it shows. Never answers, never touches records | `/shape <address> <words>`, or `/<app>.<card> <words>` |
| **Do** | Runs one of an app's own commands: the only way records change | `/<app> <words>` |
| **Forma** | Makes something new: picks or builds an app | A plain sentence, `/forma <words>`, or a folder's formation: `/<folder> <words>`, **Make a card**, **✨ Make an app** |

**Why "Forma" is not the reshape verb.** *Forma* is Latin for "shape" and "form", but the word is kept for making things: "describe it and Forma builds it". Using it for restyling too would blur two jobs into one word, and a verb should explain itself without a tour. So Forma makes things, and Shape changes their form afterwards.

## What a canvas holds

| Kind | What it is | Contains | Governed? | Examples |
| --- | --- | --- | --- | --- |
| **App** | A team tool with its own data, permissions, approvals and receipts; shareable | **Cards**, its views | Yes | Expenses, Time off, Purchase requests |
| **Card** | One view of an app: a number, chart, table, records list, form or ledger | — | Yes, through its app | `expenses.ledger`, `expenses.total` |
| **Widget** | Your own ad hoc tool, with no governed data | — | No | Weather, Calculator, Clock, Note, Folder |
| **Arrangement** | How things sit; layout only | Cards | No | Stack, Unite frame, Focus |

- A card always belongs to exactly one app. Remove the app and its cards go with it.
- A **folder** is a widget that holds files. Its one power, **formation**, *creates an app* from them (**✨ Make an app**), or a minimal app with one card (**Make a card**), and that app links back to the folder. Its **Ask** reads its files and changes nothing (APP-124).
- A **note** is a widget. It is saved privately for you. Shaping a note changes only its look; you edit its text in place.
- Arrangements are never listed in the Library; they only change how things sit.

## Start from your data (APP-124)

Data → card → app is one visible path, with no new concepts: a folder holds the files, Ask reads them, formation makes a card or an app.

1. **Bring the files.** On an empty canvas, **Drop files or upload** (beside **Describe what you need** and **Browse your library**) opens a file picker; dropping files anywhere on the empty canvas does the same. A new folder holds them, its name ready to type. "+ Folder" and dropping onto a folder work as before.
2. **The folder offers three buttons** as soon as it holds files (Ask for anyone who can read it; the two Make buttons for its owner):

   | Button | What it does | Changes |
   | --- | --- | --- |
   | **Ask** | Opens the Ask tab scoped to the folder ("Asking about 📁 Q3 board pack (its tables)"): a question about its CSV and XLSX files | Nothing |
   | **Make a card** | Formation constrained to one card: a minimal app named after the folder (or the file, for Uploads or an untitled folder) with exactly one card, the most useful view of its data, linked back to the folder ("Built from folder"); it gets an address when one is free | A new app, after the confirm |
   | **✨ Make an app** | The full formation (unchanged): an app from the folder's files, from your words | A new app, after the confirm |

   Both Make buttons open the folder's confirm first (Make a card says what it will make; Make an app shows its editable prompt). Nothing is created before **Make card** or **Build**. **Brief me** stays beside them.
3. **The card leads to the app.** A card made from a folder is a small app like any other: it shows its address chip (`/q3-board-pack-card`), you can Ask about it, Shape it, and make the full app from the same folder later.

**As built (APP-124), where the details were open:**

- **Ask on a folder** reads the folder's tables (its CSV and XLSX files: confirmed ones first, then the newest; at most 3, the first 200 rows of each) under the folder's own authorization, in one read-only snapshot, and writes nothing (not even an audit row). The answer is text plus every row it used, each with its provenance from the server: the file (name and the SHA-256 of its bytes), the sheet (XLSX) and the row (the n-th row under the header); **Open file** shows it. A table longer than 200 rows makes the answer partial, and it says so ("at least", "not the full total"). PDFs and images are not tables: **Brief me** reads PDFs. A folder answer has no **Pin to canvas** or **Make it a card** (those keep record receipts); **Make a card** on the folder is the way to a card. Someone who cannot read the folder gets the same "not found" as for a folder that does not exist, and no AI call is made.
- **Without the AI model** Ask on a folder answers only the demo organization's rehearsed questions, computed from the rows ("how many", "what's the total", "what's in it"; a change request is told that Ask only reads); anything else says the model is not configured.
- **Make a card** picks its one card by rule, not by the model, so it works in every organization: a chart of the first number over the first date (else text) column when the table has both, else its totals, else its rows. It reads the folder's first table (confirmed first, then the newest). Its address is the app's name in lowercase with hyphens; when the folder (or a verb, a library app or one of your names) already answers to it, `-card` is added, then `-2` … `-9`, else the app stays unnamed.
- **The empty-canvas upload** lands the files and stops: no data card is placed and no column sheet opens; the folder's buttons are the next step. Files dropped on a folder, or on a card (they go to Uploads), behave as before.
- **Deferred:** merging cards from different apps into one app (cross-app merge) is not built; a card belongs to exactly one app, and combining two apps is a new app (Forma).

## The right pane

| Tab | Shows | Opens with |
| --- | --- | --- |
| **Library** | **Apps** (category → app → cards) and **Widgets** | `L`, `/apps [query]`, the Library button |
| **Commands** | Every verb, with its arguments and an example | `/commands`, `/help` |
| **Ask** | A read-only conversation about your data (APP-122); `@` scopes it, its chip's × asks about the canvas (APP-123) | `/ask …` or `?` + text in the bar, ⋯ → Ask about this |
| **Shortcuts** | Keys and gestures | `/shortcuts`, ⋯ → Shortcuts |

Every Library row shows:
- a dot: filled when the item is on your canvas, hollow when it is not;
- "n of m cards" on app rows;
- an **Add / Remove** button;
- its **`/name`** in grey, with "also /…" when it has aliases.

The pane always reflects the canvas.

## Names and addresses

- Every app and card has a **name** (what you read) and an **address** (what you type). An app's address is its name in lowercase with hyphens: Expenses → `/expenses`. A card's address is `app.card`: `/expenses.ledger`. Every card also has a number in the Library: `3.2`.
- **Two forms, like `/plugin:skill`** (APP-121, 2026-10-31). The **full form** always works and is never ambiguous: `/app:<name>` (an app, by its address, an alias or your own shortcut), `/card:<app>.<card>` or `/card:<number>`, `/widget:<kind>`, `/folder:<name>`. Words after work the same: `/app:expenses $86 Nobu` is `/expenses $86 Nobu`, and `/card:expenses.ledger <words>` is `/expenses.ledger <words>`. The **short form** (`/expenses`, `/expenses.ledger`, `/weather`, `/q3`, `/e`) works when it is unambiguous.
- **A bare name reaches a verb first**, then your names: your apps, then your card shortcuts, then your folders (APP-123). An app whose address equals a verb or a verb's alias is reached by its full form: `/notes` is the Note widget, `/app:notes` is the Notes app. A folder is reached by its name in lowercase with hyphens (Q3 board pack → `/q3-board-pack`) or its number; when a verb, an app or a card shortcut takes that name, `/folder:<name>` always reaches the folder.
- Typing `/app:` lists your apps, `/card:` your cards, `/widget:` the widgets, `/folder:` your folders.
- The address is shown wherever the thing appears: the Library row, the card header (a `/expenses` chip; click it to put it in the bar), the hint while you type, and the confirm card ("via /expenses"). It shows the short form when that is unambiguous for you, else the full one (`/app:notes`); your own shortcut reads "/e → app:expenses (your shortcut)" in the `/` list.
- **Aliases** are extra addresses that also work, and only the system makes them: the catalog's own (`/expense` of `/expenses`, kept forever) and the old address an owner renames away from (kept automatically). Nobody edits an organization alias; your own shortcuts are yours alone.
- **Where you name things** (APP-121): the ✎ on a Library app or card row (name, address, description, and your own shortcuts); double-click a card's title on the canvas; `/rename` and `/alias`. Someone who may not rename a card for everyone gets their own name for it; the app itself is renamed only by its owner or an admin.
- **Who can rename** (APP-121):

  | What | Who | Seen by |
  | --- | --- | --- |
  | App name, address, description; card names and descriptions | The app's owner or an admin | Everyone who can open the app |
  | Personal shortcuts (`/exp` or `/e` → Expenses, `/q3` → a card) | You | Only you |

- **Naming rules:**
  - Addresses are unique in an organization, regardless of letter case.
  - Nothing may take a verb's name or a verb's alias (`/card`, `/app`, `/agents`, `/keys` …).
  - **An app's address never takes a name another app answers to**: another app's address or any of its aliases (hidden or not), another app's current or kept address, a saved app's `/name`. Catalog names always stay with their app: `/spend` stays Vendor spend's, so Expenses is renamed to `/team-spend`.
  - An app's display name is never another app's name or display title.
  - Only a library app or an app shared beyond its owner gets an address: a private app cannot hold a name.
  - A personal shortcut may be a single letter, never an app's address or alias, a saved app's name or a verb, and one the organization later takes stops working.
  - Renaming keeps the old address working, so nothing breaks. The catalog's own names (`/expenses`, `/expense`) always stay.
  - Renaming a card changes only its label, never its data or receipts; only a card the app has can be named, and you keep at most 100 names and shortcuts of your own.
  - Every change is audited.

## Verbs (the Commands tab)

| Group | Verb | Does |
| --- | --- | --- |
| Place | `/app <name or number>` | Puts an app and its cards on the canvas; also `/app:<name>` (the full form) |
| | `/card <address, number or words>` | Puts one card on the canvas: `/card expenses.ledger`, `/card 3.2`, `/card spend by category`; also `/card:expenses.ledger` |
| | `/widget <weather, calculator, clock, note or folder> [details]` | Puts a widget on the canvas: `/widget weather Austin`, `/widget:weather Austin` |
| Arrange | `/arrange` | Lays the canvas out, one row per app |
| | `/unite <app>` · `/split` | Assembles an app's cards into one frame, and takes them apart again |
| | `/focus` | Opens the selected card large, in focus mode |
| Make | `/forma <words>` | Makes something new from your words: exactly a plain sentence (picks or builds an app, behind its confirm card) (APP-123) |
| | `/<folder> <words>` · `/folder:<name> <words>` | Builds an app from a folder's files (formation): `/q3 build a metrics app`, `/folder:q3 brief me`. The long form `/folder <name> formation <instruction>` still works (APP-123) |
| | `/rename <app or app.card> <new name> [/address]` | Names an app (and gives it a new address) or a card: `/rename expenses Team spend /team-spend`, `/rename expenses.total Q3 spend` (APP-121) |
| | `/alias <shortcut> <app or app.card>` | Your own shortcut: `/alias exp expenses`, `/alias e expenses`, `/alias q3 expenses.total`; `/alias exp` alone removes it (APP-121) |
| Shape | `/shape <address> <words>` | Changes how a card, an app (its theme) or a widget looks or what it shows (APP-122). The address takes every form: `expenses.ledger`, `card:expenses.ledger`, `app:expenses`, `widget:calculator` |
| Ask | `/ask [app or app.card] <question>` | Opens the Ask tab with your question; read-only (APP-122). Name an app or card first to ask about it (`/ask expenses why did spend jump in March?`, `/ask expenses.ledger which claims are overdue?`), or `@`-mention it anywhere in the question (`? @expenses.total why is it high?`, APP-123); otherwise it asks about the selected card, else the canvas. `? <question>` is the quick form |
| Help | `/apps [query]` · `/commands` · `/shortcuts` · `/agent` | Library · Commands · Shortcuts · Connect an agent |

Old shortcuts keep working without being listed: `/weather`, `/calculator`, `/clock`, `/note`, `/folder` on its own, `/folder <name> formation …`, and `/help`. Verbs are always bare; the space forms `/app <name>`, `/card <x>`, `/widget <kind>` are the same as the full forms `/app:<name>`, `/card:<x>`, `/widget:<kind>`.

## Ask and Shape (APP-122)

Ask and Shape are the two AI jobs that work on things already on your canvas. **Where you type decides the job, never the AI.** A question cannot change a card, because the Ask job has no way to change anything.

### How to ask

Ask is a conversation about your data, like talking to an analyst. Every way in opens the **Ask** tab of the right pane:

| You type or click | It asks about |
| --- | --- |
| `/ask expenses why did spend jump in March?` | The app: all its records |
| `/ask expenses.ledger which claims are overdue?` (or `/ask 3.2 …`) | That card: only its records |
| `/ask app:expenses …` · `/ask card:expenses.ledger …` · `/ask e …` (your shortcut) | The same, in the full form or by your own name |
| `/ask why is this high?` | The first word names nothing, so the whole line is the question: the selected card, else the canvas |
| `? where are the red invoices` | The quick form: the selected card, else the canvas |
| `? @expenses.total why is it high?` · `? why so many @app:expenses` · `@card:expenses.ledger …` · `@q3 …` (your shortcut) | The app or card the `@` mentions, anywhere in the question (APP-123). One mention per question |
| `?` on the canvas | Puts `? ` in an empty bar (it only focuses the bar when you have half-typed something) |
| A folder's **Ask** button, or `?` with a folder card selected | The folder's tables (APP-124): each row it used, with its file and row |
| ⋯ → **Ask about this** on a card | That card |
| The Ask tab's own input | The scope its chip shows |

- **What comes back:** a short answer in text, and the records it used, each with its receipt (the record, its version and commit, the app's publication and definition). A number it computes (a count, a total) lists every record it counted.
- **What never changes:** records, cards, the canvas. Ask runs in a read-only transaction. Asked to change something, it answers with what to type (`/shape expenses.ledger …`); it never switches jobs.
- **The scope** shows as a chip at the top of the tab ("Asking about /expenses.ledger"). A verb is never an address here: `/ask card why…` asks "card why…". The chip is removable when it names an app or a card: its **×** makes the conversation about the whole canvas (APP-123). A mention sets the chip for the questions that follow.
- **Pin to canvas** on an answer: a read-only answer card labeled **"As answered on <date>"**, with the question, the text and every record it used with its receipt. It never runs again, and its text is never derived again, so it never passes as a live, receipted answer: ask again for today's answer. It is yours alone (it is never added for everyone), stays across sessions while you can still read every app it cites, and has **Remove** (APP-123).
- **Make it a card** on an answer whose records come from one list: a Shape preview of a new card showing exactly those records (the answer's record set), each with its receipt, and a small card above the bar: **Add as new card** or **Cancel**. Nothing is added until you click; then it is a card made by you (see How to shape) (APP-123).

### How to shape

Shape changes how a card, an app or a widget looks or what it shows. Every way in ends at a **preview**:

| You type or click | It shapes |
| --- | --- |
| `/shape expenses.ledger only red invoices` | That card's view |
| `/expenses.ledger only red invoices` (the dot shortcut) · `/card:expenses.ledger only red invoices` (its full form) | The same card, the same way |
| `/shape expenses dark green, compact` · `/shape app:expenses …` | A theme on every Expenses card |
| `/shape calculator make it a TI-84` · `/shape widget:calculator …` | The calculator's look |
| ⋯ → **Shape…** on a card, an app frame or a widget | Puts `/shape <its address> ` in the bar for your words |
| A widget's own **Shape** field (formerly Restyle) | That widget, applied at once |

- **What comes back:** a preview on the item, and a small card above the bar: **Apply** (replace the view), **Add as new card** (cards only) or **Cancel**. Applied, the item is labeled *shaped* with **Revert**.
- **Add as new card** makes a **card made by you** (APP-123): it belongs to its app, reads "Made by you from /expenses.ledger · only you see it", comes back on every visit and stays live over its records. **Remove** (click twice to confirm) deletes it for good, audited; × only takes it off the canvas until your next visit. The app's owner or an admin sees **Add for everyone** on their own cards: every member who can read the app's data then sees it ("added for everyone by the app's owner"), and **Keep for me only** takes it back; both are audited. On someone else's card added for everyone, the app's owner or an admin sees **Keep for its maker only** and **Remove**, both audited. You keep at most 100 cards and pinned answers.
- **What never changes:** records. Shape never answers in words. A view may filter or hide records; it never removes receipts, and every number it computes traces to its records.

### Why not `/expenses:ask`?

The colon always means *kind:name*: `/app:expenses`, `/card:expenses.ledger`, `/widget:calculator`, `/folder:q3`. It names a thing, never an action. Jobs are verbs and come first: `/ask expenses …`, `/shape expenses …`. So `/expenses:ask` is not a form; type `/ask expenses …`.

| | **Ask** | **Shape** |
| --- | --- | --- |
| What it is | A conversation about your data, like talking to an analyst | Changing how a card, an app or a widget looks or what it shows |
| Where | `/ask [address] <question>`, `?` first in the bar, ⋯ → **Ask about this**, the Ask tab | `/shape <address> <words>`, `/<app>.<card> <words>`, ⋯ → **Shape…**, a widget's Shape field |
| Examples | "where are all the red invoices?" · "why did spend jump in March?" · "summarize Q3" | A card: `/expenses.ledger highlight overdue in red`. An app: `/shape expenses dark green, compact`. A widget: `/shape calculator make it a TI-84` |
| What comes back | Text with receipts and links to the records it used | A preview: **Apply** (replace this card) or **Add as new card**. Nothing changes until you click |
| Can it change anything? | **No.** It runs read-only: it cannot change records, cards or the canvas | Only the look and the view: chart type, colors, columns, filters, the math shown. **Never records** |
| Scope | The app or card you named, else the selected card, else the whole canvas. A chip shows which | The item you addressed |

**What Shape changes, by target:**

| Target | Shape changes | Example |
| --- | --- | --- |
| **A card** | Its view: chart type, colors, columns, filters, the math shown | `/shape expenses.ledger only red invoices`, or the shortcut `/expenses.ledger only red invoices` |
| **An app** | A theme across all its cards: colors, accent, density. Each card keeps its own view | `/shape expenses dark green, compact` |
| **A widget** | Its look | `/shape calculator make it a TI-84` · `/shape weather minimal` |

An app has no look of its own; its look is its cards, so shaping an app themes them all. A widget is a one-off of yours with no records, so shaping it only restyles it. A note keeps its text; only its look changes.

**The dot is a shortcut for cards.** `/<app> <words>` (no dot) talks to the app: Do. `/<app>.<card> <words>` (a dot) shapes that card. `/<widget> <details>` places a widget (`/weather Austin`); to restyle it, use `/shape <widget>`. These are the short forms; the full forms say the same thing and are never ambiguous: `/app:<app> <words>`, `/card:<app>.<card> <words>`, `/widget:<kind> <details>`.

**The overlap is only buttons you click:** ⋯ → **Ask about this** on a card opens a scoped Ask, and nothing changes. **Make it a card** on an answer opens a Shape preview of a new card over the answer's records; nothing is added until you click **Add as new card** (APP-123).

**Mis-routes are harmless.** `/expenses.ledger $86 Nobu` gives a preview nobody applies; `/expenses highlight red` matches no app command, so the confirm card stops it.

**The rest of the bar:**
- **Do:** `/<app> <words>` runs one of the app's own commands (`/expenses $86 Nobu, Sarah approves`). The confirm card names the command before any record changes.
- **Forma:** a plain sentence, or `/forma <words>` (exactly the same), picks or builds an app; `/<folder> <words>` (or `/folder:<name> <words>`, or the long `/folder <name> formation …`) builds one from a folder's files. Both confirm first. A question typed as a plain sentence goes to Forma and stops at its confirm card; type `/ask` or `?` first to ask.
- **Place:** `/<name>` on its own puts the thing on the canvas.

**As built (APP-122), where the details were open:**

- **"Red"** has no color in the data: Ask and Shape read it as *needs attention*, a record waiting on a decision (submitted, pending, requested, open, overdue, unpaid) or refused, disputed or flagged. "Overdue" is waiting on a decision.
- **Ask's scope** is the app or card `/ask` names first, else the selected card, else every app on the canvas (at most 12). A card reads only its own collection's records (a view card reads its app's). Everything is read in one read-only transaction; each receipt names the record, its version and commit, and the app's publication and definition. An address you cannot read gets the same "not found" as one that does not exist, and no AI call is made.
- **Only the newest 200 records of each app are read.** When an app has more, the answer says so ("among the newest 200 …"), a count reads "at least", and a total is never presented as the total ("Not the full total: …"). A shaped card says "From 12 of the newest 200 records loaded; older records exist and are not included", and its computed number is labeled "(newest 200 only)".
- **Receipts cover every number:** a count or total lists every record it counted, not a sample. Money is added within one currency only: two currencies show two amounts, never one sum. An average is rounded half-even to the currency's minor unit and says "(rounded)" when it is not exact. Decimals compare and add at their own scale (123.45 is 123.45).
- **A shaped card stays live:** when its records change, its view is computed again over the new data (an applied card and a card added with **Add as new card** alike).
- **A second preview** on the same item replaces the first; cancelling the first never undoes the second. **A second question** asked while one is being answered waits its turn.
- **What a card's Shape runs over:** a records or ledger card, its records (each row keeps its record's receipt; a grouped or summed number lists the records it came from); a view card, its released result (the query's receipt stays on the card). Shaping an app card themes its app. A form or a note has no data to shape, and **notes and folders cannot be shaped yet**.
- **`/<app>.<card>` with no words** places the card, as `/card` does.
- **A widget's Shape field** applies at once, as Restyle did; `/shape <widget>` previews first, like every other `/shape`.
- **Without the AI model** Ask and Shape answer only the demo organization's rehearsed sentences (APP-098: "where are the red …", "how many", "what's the total"; "only red invoices", "highlight overdue in red", "total by month", "dark green, compact", "make it a TI-84"); anything else says the model is not configured.

### Three layers, and what may change each

| Layer | What it is | Changed by |
| --- | --- | --- |
| **Records** | The app's governed data | **Do** (the app's own commands) or **Forma** (a new app from files), always confirmed |
| **Cards, app themes and widgets** | Views: which records, which math, what look. They store no data and read live, with receipts | **Shape**, previewed and applied by you. A shaped item is labeled *shaped* and has **Revert** |
| **Answers** | What Ask tells you | Nothing; read-only |

- "Make me a new card from this data in the shape of …" is Shape, not recording data. It's a new view over the same records, and every number still traces to its source records.
- A card made from an app's data belongs to that app. It is yours ("made by you") and persists until you remove it; the app's owner or an admin can add their own for everyone who can open the app (APP-123). It stores only its view, never the records: every number is read live, with its receipts.
- A card that combines two apps (expenses with time off) is a new app: Forma, deferred (cross-app merge, APP-124).

**As built (APP-123), where the details were open:**

- **What a card made by you stores:** its app and work, the card it came from, its address and title, and its closed view spec, judged by the server against the work's published schema (field ids, types and enum values from the schema, never the list the canvas sent; `"receipts": false` is refused). A card made from an answer stores the answer's record set (the records' ids, at most 200) instead of a filter; it shows those records only, and says when some are no longer among the newest records read.
- **What a pinned answer stores:** the question, the text, the scope, the references and their receipts, exactly as Ask answered them, and the apps' works it read. The server checks every receipt before storing it: the record revision exists, and the publication is the one that revision was read under (its work's own), with that definition digest; a receipt storage never issued is refused. The text is not derived again: the card says **"As answered on <date>"** and never runs again, so it is a record of what Ask said then, not a live answer. An answer with no records pins too.
- **Who sees what:** a card or a pinned answer is listed only while you can read the data it shows: you can open its app and read its records (the same check as Ask's read: your access to the app's work, the app, and every source the work reads). Lose that, and it disappears from your canvas; get it back, and it returns. Your cards and answers are yours; a card added for everyone is seen by every member who can read its app's data, while its maker is still a member of the organization. Another organization's member, or anyone without a session, gets the same "not found" as for something that does not exist.
- **Who changes what:** the maker removes their card or answer, and adds their card for everyone (when they own the app or are an admin) or keeps it for themselves. The app's owner or an admin can take anyone's card added for everyone back to its maker, or remove it, even when its maker has left; a private card stays its maker's alone. Every change is audited; removing keeps the row (a soft delete).
- **`@` in the bar:** in an Ask line (`? …`, `/ask …`) an `@` mentions an app or a card, never a folder; outside one, `@` still mentions a folder for Forma.
- **Not done yet:** the `@` list does not suggest apps and cards while you type; a card made by you has no Library row (it is placed on every visit; × only hides it until then).

## Adding and removing

- **Add:** the Library's **Add** button, the verbs above, or a sentence. Clicking and typing always do the same thing.
- **Remove:** the **×** on a card or widget, **Delete** on the selection, or the Library's **Remove** button. Removing only takes something off your canvas; it never deletes data. There is no `/remove`. A card made by you or a pinned answer also has **Remove**, which deletes it for good (it holds no record; APP-123).

## Shortcuts that are not obvious

| Keys | Does |
| --- | --- |
| `?` | Puts `? ` in an empty bar, ready to ask; with something half-typed it only focuses the bar (APP-122). `/shortcuts` opens the Shortcuts tab |
| `L` | Library |
| `F` · `U` · `Shift+U` | Focus, unite, split the selected card or app |
| `V` · `H` | Move tool, hand tool |
| Space + drag · scroll | Pan |
| ⌘/Ctrl + `=` · ⌘/Ctrl + `−` · pinch | Zoom in, zoom out |
| `Shift+0` · `1` · `2` | Zoom to 100%, fit everything, fit the selection |
| ⌘/Ctrl + `G` · ⌘/Ctrl + `Shift+G` | Stack the selection · spread a stack |
| ← / → on a stack | Flip through it |
| Enter · Esc on a stack | Fan it out, fold it back |
| Arrow keys · Shift + arrow | Move the selection 1 px · 10 px |
| Double-click a stack's name | Rename the stack |
| Double-click a card's title | Rename the card (for everyone if you own the app, else for you); Enter saves, Esc cancels |

No shortcut fires while you are typing.
