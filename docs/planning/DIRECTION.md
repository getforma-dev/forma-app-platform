# Where work starts

Victor's November 1 direction is the app as a page within a clean new-user trial. Read the [pivot review](PIVOT-REVIEW-2026-11-01.md), [first-run decision](first-run/README.md), and the app-model notes ([app](app-model/app.md), [components](app-model/components.md), [widget](app-model/widget.md), [folder](app-model/box.md)). These separate intended behavior from current code and preserve app/publication/work identity.

The November 1 sharing clarification prioritizes **APP-154** now: named live app guests, separate platform invitations, and optional connected canvas placement ([ADR-014](../architecture/ADR-014-LIVE-APP-SHARING.md)). APP-139 consumes this implementation for its complete first-run trial. The earlier sequence is retained below for context:

1. **APP-145**, the prerequisite for a real first member: one authoring shell on intentional compose, with no shell created by discovery/listing. PR 25 exists; review repairs are not a deployment.
2. **APP-141.** Create before placing. PR 26 currently supplies description examples and generation, not admitted-template instantiation; do not label those examples as templates.
3. **APP-142.** The united app is the page on the canvas. Focus returns to it; Fan out returns its parts to the board. PR 27 and its review now include successful owner-created sharing/live updates and narrow-layout evidence.
4. **APP-144.** Extend the authorized `/w/{app}/{work}` renderer into the same usable app document. Preserve the exact saved work. No public anonymous page is implied.
5. **APP-143.** Ask about the selected app, focused part or widget. Context grants no authority.
6. **APP-139.** Qualify the whole empty-user journey, including a share that causes the recipient to sign up, page/canvas placement and measured live updates. APP-145 and APP-144 are explicit dependencies. Active-member sharing alone does not pass it.
7. **APP-140.** Match the clean local/live world, provision real panel accounts and capture the actual path within three minutes. Read-only observation is complete; correction of beta and hosted acceptance are not.

Implementation, review, landing and deployment are separate states. These tickets remain Backlog while their older required predecessors are not Done. The dependency Order values follow the sequence above; APP IDs and historical logs are unchanged. Do not use `board.py next` alone to select the pivot work or mark acceptance from an open PR.

Stacks, focus and folders retain APP-104, APP-106 and APP-120 contracts. Detaching a component while keeping its data is undecided. APP-150–153 in [LATER.md](LATER.md) remain later; the recording does not require those enhancements. The original platform roadmap and phase gates still apply after the trial.
