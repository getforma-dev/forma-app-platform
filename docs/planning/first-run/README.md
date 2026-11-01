# First-run trial

Victor, 2026-11-01: a demo that opens on a canvas already full of apps and records does not count. A reviewer can call that staged. The trial is a link a new paid user opens, as a person who just signed up. It does not start halfway through someone else's work. The recorded video of that path cannot run past three minutes. The link itself is not on a timer. Someone trying it may take longer than the recording.

The product for that path is already here: an invite and a sign-in, an empty canvas, compose through admission, one audited record, and a share between people who are already members. The trial starts with one invite into an empty organization, not a public signup. One person is on the canvas. They build the app and share it. The other person does not have an account yet. Accepting that share is what makes them sign up. They arrive with no other apps, and the shared app is the one both of them work from.

This note classifies the board. It does not move tickets, close a phase, or delete the seeded demo. `docs/pitch/SCENARIOS.md` and APP-098 stay the record of the concept-car rehearsal. APP-097 still walks that ledger later. They are not what a reviewer is handed.

## The path

1. The first invite admits one new member into an organization with no library apps, no placed cards, and no records.
2. The canvas is empty. A short guide on that canvas names the next moves. It does not use `GENUI_DEMO_MODE` or a rehearsed sentence.
3. The member describes one app. It appears through the existing composer and admission path. The member adds one record through the existing command path. A shown number carries its receipt.
4. The member shares that work. The recipient does not already have an account. Accepting the share is what makes them sign up and sign in. They arrive with no other apps, and this shared app is the one both members work from. A stranger gets the uniform 404. Today's `share_work` still refuses a grantee who is not already an active member of the organization (`app/src/storage/sharing.rs`). Closing that gap is part of APP-139.
5. The evidence is a sanitized click log, plus a recording of this same path that stays within three minutes. No credential, invite token, or model key. The try-it link is not cut to fit the recording.

## The hole

A generated or rule-built definition is published onto an app the member already owns, with `edit_definition` and `run` (`app/src/storage/composer.rs`, `choose_authoring`). The clean dev stand creates that shell with `create-app` before the persona signs in (`app/src/dev/db.rs`). An invitation does not. A new member of an empty organization therefore has no authoring app, and compose answers `no_authoring_app` (`app/src/boxes/routes.rs`). That is the first slice of APP-139. It is not a new permission model, a server-side layout store, or a second demo mode.

## Two machines, not one canvas

The pitch profile on the main checkout (`127.0.0.1:4001`–`4003`, Diego, Sarah, Priya, the seeded library) is the test sandbox. It stays for development. It is not deployed.

The local twin of the AWS trial is the main checkout's clean profile, `127.0.0.1:4010`. That port is the empty world. Victor records the demo there. A new user and each panelist sign in on the same kind of world, on the live site that this port matches. A worktree clean port is not that twin. APP-140 is the ticket.

## Pile A — keep

The product a new user needs. These stay where they are until their own acceptance is met.

| Ticket | What it already is |
| --- | --- |
| APP-009, APP-074 | Sign-in and an invite |
| APP-092, APP-104 | The canvas and placing a card from the library |
| APP-093 | Describe an app; the definition is admitted |
| APP-012, APP-013, APP-094 | A record, its outcome, and a receipt |
| APP-095 | Share that work with one other member |

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
