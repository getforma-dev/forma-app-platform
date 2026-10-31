# Canvas rules: what is on a canvas, how you talk to it, how things are named

This is the single reference for the canvas model and its grammar. The decisions are recorded in APP-120 (model and grammar), APP-121 (naming) and APP-119 (commands versus apps). Where this page and the code disagree, the code is wrong: file a ticket.

## The one sentence

**Type `/` and a verb to change the canvas, `/` and a name to tell that thing something, or just type a sentence.**

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
| | `/rename …` · `/alias …` | Names things (APP-121) |
| Help | `/apps [query]` · `/commands` · `/shortcuts` · `/agent` | Library · Commands · Shortcuts · Connect an agent |

Old shortcuts keep working without being listed: `/weather`, `/calculator`, `/clock`, `/note`, `/folder` on its own, and `/help`.

## Talking to things

- **`/<app> <words>`** tells an app something. The app's own definition decides what the words may do: `/expenses $86 Nobu, Sarah approves` files a claim, and `/expenses approve the Nobu dinner` decides one. You always see a confirm card that names the command ("Submit claim") before anything is saved.
- **`/<app>` on its own** puts the app on the canvas, the same as `/app <app>`.
- **`/<app>.<card> <words>`** reshapes one card: `/expenses.by-category last quarter only`. The AI proposes a typed change, it is checked, and the card is labeled *adapted*. This is planned: the next step after APP-121.
- **A plain sentence** with no `/` goes to the AI: it picks the right app, or builds one.
- **`@folder`** in a sentence points at one of your folders: "build a metrics app from @Q3 metrics".

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
| ⌘/Ctrl + scroll · pinch | Zoom |
| `Shift+0` · `1` · `2` | Zoom to 100%, fit everything, fit the selection |
| ⌘/Ctrl + `G` · ⌘/Ctrl + `Shift+G` | Stack the selection · spread a stack |
| ← / → on a stack | Flip through it |

No shortcut fires while you are typing.
