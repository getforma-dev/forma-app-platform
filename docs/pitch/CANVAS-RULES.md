# Canvas rules: what is on a canvas, how you talk to it, how things are named

This is the single reference for the canvas model and its grammar. The decisions are recorded in APP-120 (model and grammar), APP-121 (naming), APP-122 (talking to things) and APP-119 (commands versus apps). Where this page and the code disagree, the code is wrong: file a ticket.

## The one sentence

**Type `/` and a verb to change the canvas, `/` and a name to tell that thing something, or just type a sentence.**

Whatever you tell, the rule is the same: **answers are free, views are cheap and undoable, records only change through the app's own commands.**

## What a canvas holds

| Kind | What it is | Contains | Governed? | Examples |
| --- | --- | --- | --- | --- |
| **App** | A team tool with its own data, permissions, approvals and receipts; shareable | **Cards**, its views | Yes | Expenses, Time off, Purchase requests |
| **Card** | One view of an app: a number, chart, table, records list, form or ledger | — | Yes, through its app | `expenses.ledger`, `expenses.total` |
| **Widget** | Your own ad hoc tool, with no governed data | — | No | Weather, Calculator, Clock, Note, Folder |
| **Arrangement** | How things sit; layout only | Cards | No | Stack, Unite frame, Focus |

- A card always belongs to exactly one app. Remove the app and its cards go with it.
- A **folder** is a widget that holds files. Its one power, **formation**, *creates an app* from them, and that app links back to the folder.
- A **note** is a widget. It is saved privately for you.
- Arrangements are never listed in the Library; they only change how things sit.

## The right pane

| Tab | Shows | Opens with |
| --- | --- | --- |
| **Library** | **Apps** (category → app → cards) and **Widgets** | `L`, `/apps [query]`, the Library button |
| **Commands** | Every verb, with its arguments and an example | `/commands`, `/help` |
| **Shortcuts** | Keys and gestures | `?` (anywhere), `/shortcuts` |

Every Library row shows:
- a dot: filled when the item is on your canvas, hollow when it is not;
- "n of m cards" on app rows;
- an **Add / Remove** button;
- its **`/name`** in grey, with "also /…" when it has aliases.

The pane always reflects the canvas.

## Names and addresses

- Every app and card has a **name** (what you read) and an **address** (what you type). An app's address is its name in lowercase with hyphens: Expenses → `/expenses`. A card's address is `app.card`: `/expenses.ledger`. Every card also has a number in the Library: `3.2`.
- The address is shown wherever the thing appears: the Library row, the card header (a `/expenses` chip; click it to put it in the bar), the hint while you type, and the confirm card ("via /expenses").
- **Aliases** are extra addresses that also work. `/expense` is an alias of `/expenses`, kept forever.
- **Who can rename** (APP-121):

  | What | Who | Seen by |
  | --- | --- | --- |
  | App name, address, aliases, description; card names and descriptions | The app's owner or an admin | Everyone who can open the app |
  | Personal aliases (`/exp` → Expenses, `/q3` → a card) | You | Only you |

- **Naming rules:**
  - Addresses are unique in an organization, regardless of letter case.
  - Nothing may take a verb's name (`/card`, `/app` …).
  - A personal alias never overrides an organization address.
  - Renaming keeps the old address as an alias, so nothing breaks.
  - Renaming a card changes only its label, never its data or receipts.
  - Every change is audited.

## Verbs (the Commands tab)

| Group | Verb | Does |
| --- | --- | --- |
| Place | `/app <name or number>` | Puts an app and its cards on the canvas |
| | `/card <address, number or words>` | Puts one card on the canvas: `/card expenses.ledger`, `/card 3.2`, `/card spend by category` |
| | `/widget <weather, calculator, clock, note or folder> [details]` | Puts a widget on the canvas: `/widget weather Austin` |
| Arrange | `/arrange` | Lays the canvas out, one row per app |
| | `/unite <app>` · `/split` | Assembles an app's cards into one frame, and takes them apart again |
| | `/focus` | Opens the selected card large, in focus mode |
| Make | `/folder <name> formation <instruction>` | Builds an app from a folder's files |
| | `/forma <words>` | Asks the AI about, or builds from, your whole canvas (APP-122) |
| | `/rename …` · `/alias …` | Names things (APP-121) |
| Help | `/apps [query]` · `/commands` · `/shortcuts` · `/agent` | Library · Commands · Shortcuts · Connect an agent |

Old shortcuts keep working without being listed: `/weather`, `/calculator`, `/clock`, `/note`, `/folder` on its own, and `/help`.

## Talking to things: the address is the scope, the words go to the AI

`/<anything> <words>` sends your words to the AI, limited to that thing (APP-122):

| You type | The AI sees | Example |
| --- | --- | --- |
| `/forma <words>`, or a plain sentence | Your whole canvas and library | "build me a vendor tracker" |
| `/<app> <words>` | That app: all its records and cards | `/expenses why did spend jump in March?` |
| `/<app>.<card> <words>` | That one card and its data | `/expenses.total add the math for me` |
| `/<folder> <words>` | The folder's files | `/q3-metrics turn this into a metrics app` |
| `/<widget> <words>` | That widget | `/calculator make it a TI-84` |

- A card question and an app question are the same feature at different widths. "Why is this number high?" fits the card; "who files the most?" fits the app.
- **`/<name>` on its own** puts the thing on the canvas, the same as `/app` or `/card`.
- **The selection is the scope.** With something selected, a plain sentence goes to it. The bar shows a removable chip ("to: expenses.total").
- **`@folder`** in a sentence points at one of your folders: "build a metrics app from @Q3 metrics".
- The old `/folder <name> formation …` keeps working as an alias of `/<folder> …`.

### Three layers, separated by what changes

| Layer | What it is | What can change it |
| --- | --- | --- |
| **Records** | The app's governed data | Only the app's own commands (file, approve), always confirmed. Plus formation, which creates a *new* app with its own records, also confirmed |
| **Cards** | A view: which records, which math, what shape | The AI, freely. A card stores no data; it reads live from the records, with receipts |
| **Answers** | An insight or a summary | Nothing. Read-only and temporary unless pinned |

**Answers are free, views are cheap and undoable, records only change through the app's own commands.**

"Make me a new card from this data in the shape of …" is not recording data. It is a new view over the same records, its math is computed live, and every number still traces to its source records.

### Four outcomes, always shown before they take effect

The AI turns your words into exactly one typed outcome. Model output is data, never code (ADR-011 I5).

| Outcome | What happens | Confirmation |
| --- | --- | --- |
| **Answer** | An insight with receipts appears beside the item. Nothing changes | None needed. **Make it a card** pins it as a card |
| **New view** | A preview of a new or changed card | **Add as new card** (default) or **Replace this card**. A replaced card is labeled *adapted* and has **Revert** |
| **Command** | One of the app's own commands ("Submit claim", "Approve") | The confirm card names the command; it runs only if the app defines it |
| **Forma** | Something new: a card set, or an app from a folder's files | A confirm card; an app from files is the one AI path that creates records |

- **The AI's guess only has to be visible and one click from right.** Chips on every result switch the outcome: Answer · New card · Change this card · Command.
- **`?` forces read-only:** `/expenses.total ? why is this high` can only answer.
- Nothing is forced onto a new canvas item first. New is the default; in place is one click.

### Where things belong

- A card made from an app's data belongs to that app, marked "made by you". Only you see it until the app's owner adds it for everyone.
- Widgets have no records, so changing one only restyles it.
- A card that combines two apps (expenses with time off) is a new app: formation, deferred.

## Adding and removing

- **Add:** the Library's **Add** button, the verbs above, or a sentence. Clicking and typing always do the same thing.
- **Remove:** the **×** on a card or widget, **Delete** on the selection, or the Library's **Remove** button. Removing only takes something off your canvas; it never deletes data. There is no `/remove`.

## Shortcuts that are not obvious

| Keys | Does |
| --- | --- |
| `?` | Shortcuts tab |
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

No shortcut fires while you are typing.
