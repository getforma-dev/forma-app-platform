# First-run trial

Victor, 2026-11-01: a demo that opens on a canvas already full of apps and records does not count. A reviewer can call that staged. The trial is a link a new paid user opens, as a person who just signed up. It does not start halfway through someone else's work. The recorded video of that path cannot run past three minutes. The link itself is not on a timer. Someone trying it may take longer than the recording.

The product for that path is already here: an invite and a sign-in, an empty canvas, compose through admission, one audited record, and private live sharing with verified app guests. The trial starts with one invite into an empty organization, not a public signup. One person is on the canvas. They build the app and share it. The other person does not have an account yet. Accepting that share is what makes them sign up. They arrive with no other apps, and the recipient follows that live app without joining the owner’s organization. Adding it to a canvas is a separate, explicit choice.

This note classifies the board. It does not move tickets, close a phase, or delete the seeded demo. `docs/pitch/SCENARIOS.md` and APP-098 stay the record of the concept-car rehearsal. APP-097 still walks that ledger later. They are not what a reviewer is handed.

## The path

1. The first invite admits one new member into an organization with no library apps, no placed cards, and no records.
2. The canvas is empty. A short guide on that canvas names the next moves. It does not use `GENUI_DEMO_MODE` or a rehearsed sentence.
3. The member describes one app. It appears through the existing composer and admission path. The member adds one record through the existing command path. A shown number carries its receipt.
4. The member shares that work. The recipient does not already have an account. Accepting the share is what makes them sign up and sign in. They arrive with no other apps and can view this exact live app, without membership in the owner’s organization. The owner may separately allow connected canvas placement. A stranger cannot read its content. APP-154 implements this path; APP-139/140 qualify real provider delivery, verification and the complete hosted journey.
5. The evidence is a sanitized click log, plus a recording of this same path that stays within three minutes. No credential, invite token, or model key. The try-it link is not cut to fit the recording.

## Review and recording

The [November 1 review](../PIVOT-REVIEW-2026-11-01.md) distinguishes trunk, open implementations and local repairs. The [recording outline](RECORDING.md) budgets 180 seconds and lists the real-path acceptance needed before recording. “Paid user” describes the intended customer experience; this trial does not implement or prove billing. Synthetic rows are allowed, but every shown row must actually be imported or committed through the product.

## First authoring transition

At the start of the pivot, a generated definition could only publish onto an app the member already owned. APP-145 now separates read-only context from explicit compose preparation and creates the first owned shell when needed, under the existing identity/entitlement fence. Publication still requires the provisioned space and owner-publish policy. The clean dev stand's operator-created shell hides that transition; APP-139 must verify an actually new member after provider signup/sign-in. PR 25 and the [pivot review](../PIVOT-REVIEW-2026-11-01.md) carry implementation, tests and delivery status. This is not a new demo mode or a server-side layout store.

## Two machines, not one canvas

The pitch profile on the main checkout (`127.0.0.1:4001`–`4003`, Diego, Sarah, Priya, the seeded library) is the test sandbox. It stays for development. It must not be what AWS serves. On 2026-11-01 a read-only check found that `https://beta.getforma.dev` is still that sandbox: demo mode on, Northstar, task definition revision 12, and the Northstar sign-in alias. Nothing was changed. APP-140 holds the reading and the still-open correction.

The local twin of the AWS trial is the main checkout's clean profile, `127.0.0.1:4010`. That port is the empty world. Victor records the demo there. A new user and each panelist sign in on the same kind of world, on the live site that this port matches. A worktree clean port is not that twin. APP-140 is the ticket.

## Pile A — keep

The product a new user needs. These stay where they are until their own acceptance is met.

| Ticket | What it already is |
| --- | --- |
| APP-009, APP-074 | Sign-in and an invite |
| APP-092, APP-104 | The canvas and placing a card from the library |
| APP-093 | Describe an app; the definition is admitted |
| APP-012, APP-013, APP-094 | A record, its outcome, and a receipt |
| APP-095, APP-154 | Same-member workflow compatibility plus private named live app guests and explicit connected placement |

## Pile B — not the trial

The seeded choreography. Nothing here is unchecked because of this note. It is no longer the trial.

| Ticket | Why it is not the link |
| --- | --- |
| APP-096 | The live rehearsal starts from personas, a seeded library, and two windows already signed in |
| APP-098 | Shortcuts whose purpose is that the stage never starts empty: personas, pre-filed claims, rehearsed sentences, the demo organization |
| APP-105 | Acceptance that runs those scenarios |
| APP-110 | Section A, written 2026-10-30, describes that seeded demo as what ships |

## Pile C — later

PH02 through PH06, APP-078's reserved database window, canvas fences, and the platform-landing list inside APP-097. Untouched by this note.
