# Canvas rules: what is on a canvas, how you talk to it, how things are named

This is the single reference for the canvas model and its grammar. The decisions are recorded in APP-120 (model and grammar), APP-121 (naming), APP-122 (Ask and Shape) and APP-119 (commands versus apps). Where this page and the code disagree, the code is wrong: file a ticket.

## The one sentence

**Type `/` and a verb to change the canvas, `/` and a name to tell that thing something, or just type a sentence.**

The AI does two separate jobs, and where you type picks the job: **Ask** answers questions about your data and never changes anything; **Shape** changes how an item looks or what it shows and never answers. Records only change through an app's own commands.

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
| **Ask** | A read-only conversation about your data (APP-122) | `?` + text in the bar, ⋯ → Ask about this |
| **Shortcuts** | Keys and gestures | `?` (outside the bar), `/shortcuts` |

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
| | `/forma <words>` | Picks or builds an app from a sentence (APP-122) |
| | `/shape <card or widget> <words>` | Changes how an item looks or what it shows (APP-122) |
| Ask | `? <question>` | Opens the Ask tab with your question; read-only (APP-122) |
| | `/rename …` · `/alias …` | Names things (APP-121) |
| Help | `/apps [query]` · `/commands` · `/shortcuts` · `/agent` | Library · Commands · Shortcuts · Connect an agent |

Old shortcuts keep working without being listed: `/weather`, `/calculator`, `/clock`, `/note`, `/folder` on its own, and `/help`.

## Two jobs: Ask and Shape (APP-122)

The AI does two separate jobs. **Where you type decides the job, never the AI.** A question cannot change a card, because the Ask job has no way to change anything.

| | **Ask** | **Shape** |
| --- | --- | --- |
| What it is | A conversation about your data, like talking to an analyst | Changing how a canvas item looks or what it shows |
| Where | The **Ask** tab in the right pane, with its own input. `?` in the bar opens it with your text | The bar: `/<card or widget> <words>`, or the **Shape** field on a card or widget |
| Examples | "where are all the red invoices?" · "why did spend jump in March?" · "summarize Q3" | `/expenses.total by month` · `/expenses.ledger highlight overdue in red` · `/calculator make it a TI-84` |
| What comes back | Text with receipts and links to the records it used | A preview of the changed item; nothing applies until you click **Apply** |
| Can it change anything? | **No.** It runs read-only: it cannot change records, cards or the canvas | Only the look and the view: chart type, colors, columns, filters, the math shown. **Never records**, and it never answers in words |
| Scope | The selection, an `@` mention (`@expenses.total`), or the whole canvas. A chip shows which, and you can remove it | The item you addressed |

**The only overlap is two explicit buttons:**
- An Ask answer has **Make it a card**. It hands the answer to Shape as a preview of a new card. Nothing is added until you click.
- Every card's ⋯ menu has **Ask about this**. It opens Ask scoped to that card. Nothing changes.

**The rest of the bar:**
- **Do:** `/<app> <words>` runs one of the app's own commands (`/expenses $86 Nobu, Sarah approves`). The confirm card names the command before any record changes.
- **Forma:** a plain sentence, or `/forma <words>`, picks or builds an app. `/<folder> <words>` builds from a folder's files; the old `/folder <name> formation …` still works. Both confirm first.
- **Place:** `/<name>` on its own puts the thing on the canvas.

### Three layers, and what may change each

| Layer | What it is | Changed by |
| --- | --- | --- |
| **Records** | The app's governed data | **Do** (the app's own commands) or **Forma** (a new app from files), always confirmed |
| **Cards and widgets** | Views: which records, which math, what look. They store no data and read live, with receipts | **Shape**, previewed and applied by you. A shaped card is labeled *adapted* and has **Revert** |
| **Answers** | What Ask tells you | Nothing; read-only. Pinning one goes through Shape |

- "Make me a new card from this data in the shape of …" is Shape, not recording data. It's a new view over the same records, and every number still traces to its source records.
- A card made from an app's data belongs to that app, marked "made by you". Only you see it until the app's owner adds it for everyone.
- A card that combines two apps (expenses with time off) is a new app: Forma, deferred.

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
