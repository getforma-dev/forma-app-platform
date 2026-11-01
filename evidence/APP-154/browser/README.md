# APP-154 browser evidence — 2026-11-01

The isolated local browser journey passed against the application, real local PostgreSQL, admitted generated app, audited record commands and native SSR pages. Test identities and book data are synthetic. Identity injection on the dedicated dev persona ports is explicit: this is product-route evidence, not hosted provider signup, invitation delivery or paid-user onboarding evidence.

- [Full journey results](browser.json): eight checks, zero browser page errors. A real record commit appeared in the shared document after **1,592 ms** and connected canvas after **2,374 ms**. Revocation cleared the document after **551 ms** and canvas after **1,112 ms**. Mobile document had no horizontal overflow.
- [Final native navigation results](final-navigation.json): four checks, zero browser page errors. Account-only and signed-out guests have no Workspace link; existing workspace members retain a working link. The final owner copy includes definition and data-source changes. One mobile dialog is present, without horizontal overflow.
- [Build identity](build-identity.json), [source hash inventory](source-sha256.txt), and [strict production build log](production-build.txt) bind the retained source and assets. Browser runs used the development stand. Production compilation separately passed all strict route budgets. The full journey preceded only the Workspace-link conditional and wording correction; the final focused run checked those changes and recaptured affected views.

The full journey covers uncertain-create retry with the same UUID and body, explicit invitation acceptance, absence of automatic placement, no guest forms or record links, denied ordinary work access, neutral wrong-identity landing, live canvas permission controls, explicit connected placement, real record propagation, offline masking/recovery, inventory-failure geometry preservation, revocation, existing-member canvas placement, and shared-only sign-out preserving the workspace session. Fault injection was limited to losing an actual committed create response and returning an inventory 503. Values, records and receipts were not replaced with test responses.

The fixture was generated and admitted through `/v1/compose` with `generate:true`; the recorded Reading List app took 11,412 ms to generate. Its create command declares initial `status=to_read`. The browser uses that admitted initial value. Selecting another enum option exposed an existing owner form-metadata gap: the backend correctly refuses it. APP-144 records the follow-up to constrain unambiguous initial-state options without relaxing command validation.

## Retained views

- [Owner sharing dialog](owner-share-created.png) and [mobile dialog](owner-share-mobile.png).
- [Recipient document](recipient-desktop.png) and [mobile document](recipient-mobile.png).
- [Account-only connected canvas](connected-canvas.png).
- [Existing member's actual workspace canvas](existing-user-canvas.png).
- [Signed-out mobile canvas](signed-out-mobile.png).

These final images were visually inspected. The owner dialog is a single viewport capture; its independently loading access list may still be refreshing in the image. Mobile tables scroll inside their cards. The guest canvas uses the same pan/zoom board primitive and keeps a connected reference to the original app.

## Reproduce on an owned disposable stand

The [full browser script](browser.mjs) and [final navigation script](final-navigation.mjs) use an isolated headless Chrome profile via a separately installed Playwright module. No personal browser profile is accessed. `PLAYWRIGHT_MODULE` may name that module's ESM path; otherwise the scripts import `playwright`. No dependency or credentials are bundled here.

Start an owned disposable stand with `GENUI_DEV_SHARED_GUEST_PORT` set to its unused guest port. Create an admitted book app with writable `title`, `author` and `status` fields and an unconditional create initial state `to_read`; save its `app_id`, `work_id` and `title` in a writable JSON file using [the example shape](fixture.example.json). Set `OWNER_ORIGIN`, `MEMBER_ORIGIN`, `STRANGER_ORIGIN` and `GUEST_ORIGIN` to the stand's Diego, Sarah, Priya and account-only guest origins. Defaults are the isolated ports 7601–7604 used for this run. The example identifiers describe the recorded stand and do not provision an app.

```sh
node evidence/APP-154/browser/browser.mjs /path/to/synthetic-fixture.json .cache/reports/app154-repro
node evidence/APP-154/browser/final-navigation.mjs /path/to/synthetic-fixture.json .cache/reports/app154-navigation-repro
```

The scripts create and revoke synthetic shares, accept invitations, add linked references and commit a synthetic book record. Use only a disposable test stand. The final run's owned stand and watcher were stopped; no owner ports or data were changed. Hosted provider/broker deployment and signup remain separate operational verification gates.
