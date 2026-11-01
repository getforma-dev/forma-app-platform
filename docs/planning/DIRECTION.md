# Where work starts

Victor, 2026-11-01. The current product direction is the app as a page. These four tickets are that direction. They stay in `Tickets/0-backlog/` because the board will not mark a ticket Selected until every ticket it depends on is Done, and those older tickets are built but not closed. Do not use `python3 Tickets/_tools/board.py next` to find this work. That command follows the old dependency queue.

Define the words before building. The spikes write `docs/planning/app-model/` and change no product code. Do them first, in this order: APP-146 what an app is, APP-147 what a component is, APP-148 what a widget is, APP-149 what a box is. Then the implementation order below. The spikes are not formal predecessors on the board, because a ticket may only depend on an earlier order number.

Start implementation in this order:

1. **APP-141.** Create the app before the canvas. Template, then description. This is the first implementation.
2. **APP-142.** The united app is the page. Focus stays. Fan-out returns the cards. The page is what is shared.
3. **APP-144.** That page is a real HTML page at its own address, rendered by us from the admitted app.
4. **APP-143.** Ask about the app, the focused part, or the widget that was clicked.

Before the description step in APP-141 can succeed for a brand-new member, APP-145 creates one authoring shell for that member. APP-139 is the trial that uses it. APP-140 is the clean world they sign in to. None of those replaces the order above.

Stacks, focus, and the folder stay as APP-104, APP-106, and APP-120 already define them. Detach-and-keep-data is not decided and is not a starting ticket.
