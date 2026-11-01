# Pitch join: canvas, planner, and composer

Status: the APP-091 record. Read against `pivot/pitch-mvp` at `1a8c9c84e889`, which descends from `phase-one-spikes` `21199f4`. The authoritative shortcut text is [APP-098](../../Tickets/0-backlog/APP-098.md). This note names the seam, the rule relaxed, and the revisit ticket. APP-097 decides keep, rework, or drop.

ADR-011's first draft named a vector index from Forma Generative UI. That index is not in this tree. The 2026-10-30 amendment replaced it: the planner is the library matcher. A generated definition enters only through admission.

## Canvas

The signed-in workspace page is `GET /canvas` (`app/src/query/canvas.rs`). A live workspace session is required. Anonymous, pre-access, expired, revoked, and foreign callers get the uniform 404, and so does a query string. The tenant is the session's. The page does not offer an organization picker.

The server renders the library as metadata and the groups the board must show without a choice (the restored run, and a record waiting for a decision). The browser places every other card. One island is registered: `IntentBar` (`app/admin/src/canvas/app.tsx`). The rest of the board is page script. It runs views through the governed query route and posts record forms through the audited command route. It does not speak SQL, admit a definition, hold the model key, or choose a tenant.

Layout and the set of cards the member brought onto the board stay in this browser (`app/admin/src/canvas/layout.ts`, DS-06 and DS-67). That is not a server record.

## Planner

There is no vector index and no client-side authority. Routing is server-side, under the member's session.

`composer::matcher` scores the member's openable library apps from the compiled catalog (`catalog.json`, produced with the library fixtures). A clear hit places that app with no model call. A near tie returns a chooser. No hit, or "build something new", goes to generation. A slash address or a chooser click pins one app and does not route (`composer::routes::Pin`). `GET /v1/library` lists only apps the session may open. A foreign tenant's module is not in that list.

The matcher runs in every organization. It is not a demo shortcut. Rehearsed sentences that skip the model are. Those are limited to organizations listed in `GENUI_DEMO_ORGANIZATIONS` and are off unless `demo::is_demo_org` is on (`app/src/demo.rs`).

## Composer

`composer::routes::compose_for` is the stable entry. The canvas uses `compose_instant` and finishes tailoring in the background. Either way, model text is data.

`Composer::generate_with` calls the model with the generation prompt and the member's source contracts. `plan_generated` parses one JSON object from the reply, pins source inputs to those contracts, admits the document whole (`admission`), requires the engine's runnable subset, then `require_authored`. A hostile or malformed reply is a refusal code. It is not rendered, not executed, and not granted. The admitted definition is published as a new revision of an app the member already holds, then saved as a work. The canvas labels that work generated.

The model key is the `ComposerModel` secret, injected into the service task. It is not an argument and not a log field.

The composer does not run SQL, emit FMIR, or write a grant. Sharing, receipts, and commands stay on their own routes after a work exists.

## Invariants

Each invariant is ADR-011 §3. The check named is the one in this tree that protects it.

| Invariant | What must hold | Check |
| --- | --- | --- |
| I1 Verified identity | A protected route requires a live workspace session. A pre-access cookie does not mint one. | `auth::tests::native_session_campaign` |
| I2 Tenant isolation | A foreign, anonymous, revoked, or pre-access caller gets the uniform 404 and does not reach the engine. The canvas routes do the same, including a query string. | `storage::query_qualification::native_query_campaign` (foreign and revoked block); `query::canvas::tests` (canvas 404) |
| I3 Receipts | A governed number is shown with its receipt. A denied or failed view shows no number. | `storage::query_qualification::native_query_campaign`; `tests::canvas_page_renders_groups_cards_and_receipts_without_javascript` |
| I4 Transactional commands | One command writes the record, the idempotency outcome, and one audit event, or it writes nothing. Retry returns the stored outcome. | `storage::command_qualification::native_command_campaign` |
| I5 Model output is data | A generated definition is admitted whole. Authoring rules run after admission. Model text is never code, SQL, markup, or a grant. | `composer::plan::plan_generated` then `require_authored`; `composer::tests::hostile_outputs_are_refused_by_admission_and_retries_carry_codes_only`; `composer::tests::generated_content_meets_the_authoring_verdict_and_offers_an_entry_form` |
| I6 No secrets in evidence | Campaign reports and this note do not carry a credential, a raw code, or a model key. | The invite and pitch campaigns scan their reports; the key is read from the secret, not from argv |

## Shortcuts

APP-098 is the register. Every entry below is one of its `DS-*` rows. The full wording, the file, and the reason are in that ticket. APP-097 revisits every row. Where a row also names another ticket, that ticket owns the proper fix.

The identifiers are the register's. They are not a continuous count.

| ID | Class | Rule relaxed | Revisit |
| --- | --- | --- | --- |
| DS-01 | Planned shortcut | Canned composer and action responses for rehearsed utterances: a fixed table of known demo intents answers with the prepared composition instead of a live… | APP-043 (model path), APP-097 |
| DS-02 | Planned shortcut | Seeded demo personas with display names and pre-existing expenses, so the canvas opens on a populated Spend Review and a teammate with a real-looking name. | APP-097 (profile: PH02 Track C) |
| DS-03 | Planned shortcut | Model-failure deterministic fallback: when the router or generation call fails, times out or hits the rate limit, the composer answers with the nearest… | APP-043, APP-097 |
| DS-04 | Planned shortcut | Pre-warmed views: the demo views are run once before the session so first results show at once and the engine cache holds the seeded revisions. | APP-096, APP-097 |
| DS-05 | Planned shortcut | `GENUI_DEMO_MODE` switch itself: one flag, off by default in code, on for the pitch deploy; every shortcut in this register reads it and none reads another… | APP-098 (this ticket), APP-097 |
| DS-06 | Debt | View layout (card positions, camera) is kept in browser localStorage per app and work, labeled view layout only; never on the server, lost across browsers. | APP-097 (PH03) |
| DS-07 | Debt | Default canvas layout re-flows client-side as results arrive, so cards can move while the audience watches. | APP-097 |
| DS-08 | Stub, closed by APP-093 | The intent bar called a stub seam answering not connected until the composer landed. | APP-093, APP-097 |
| DS-09 | Debt | Live-update client subscribes only when the server sets `GENUI_CANVAS_EVENTS_URL`; otherwise the canvas does not update live. | APP-095, APP-097 |
| DS-10 | Debt | The vendored GenUI runtime canvas adds one public method (`moveItemTo`), and since APP-104 a handful of local changes listed in its README (batched… | APP-003 follow-up, APP-097 |
| DS-11 | Debt | Seventh `genui_admit` definer `share_work` and its A4 SELECT grants; ADR-007 D7.1 names six and D7.3 limits permission rows to `admit_redemption` and the… | APP-097 (decides), ADR-007 amendment |
| DS-12 | Debt | `permission.granted` is carried on the admission chain; ADR-007 D7.8 lists it on the operator chain only. | APP-097 |
| DS-13 | Debt (review M3) | Share copies the sharer's grants onto the teammate, including organization-wide `source_read` on every source; it is not the teammate's own permissions and… | APP-097, ADR-011 amendment |
| DS-14 | Debt | No unshare route: revoking a share is operator `revoke-permission`, one row at a time; the audit event holds only a digest of the granted set. | APP-097 (PH03) |
| DS-15 | Debt (review M1, M2) | Event stream bounds: a client that stops reading bypasses the one-hour lifetime and 30 s re-check; each event costs one fenced data-pool transaction per… | APP-097, APP-061 |
| DS-16 | Debt | Event hub is in-process: correct for one task (D7.18), wrong for multi-task deployment. | APP-061, APP-097 |
| DS-17 | Debt | Members list shows a neutral label derived from the principal id because 0002 principals carry no display name; demo personas get names only in seed (DS-02). | APP-097 |
| DS-18 | Debt | Event stream ends after one hour and on epoch change, but a session's absolute expiry and idle sweep do not close it; the stream can outlive a destroyed… | APP-097 |
| DS-19 | Debt (APP-095 m1-m4) | Sharing minors: `inserted:false` leaks the grantee's prior holdings; session-store error answers 404 not 503; grantee membership read without `FOR SHARE`;… | APP-097 |
| DS-20 | Debt | Operator-only grants for app creation: a generated or adapted definition is a new revision of an app the member already owns; a member owning none can… | APP-097 (PH03) |
| DS-21 | Debt | Composition is three transactions (draft, publish, save); a publish `Stale` or save denial leaves a drafted revision without a work; the ALB 504 case also… | APP-097 |
| DS-22 | Debt (review M2, M3, m6) | Generated and adapted works publish onto a library app and displace it in the catalog; the 50-work listing cap hides library apps after about 37… | APP-097 (fix before deploy if review order holds) |
| DS-23 | Debt (review M4) | Money and decimal columns show digit-grouped minor units (262,620.04 shows as 26,262,004); math and receipt are exact, display is 100x wrong and has no unit. | APP-097 (fix before deploy) |
| DS-24 | Debt (review M1, M5) | Compose route holds anonymous body reads up to 120 s (check order); ALB idle timeout (60 s) is below `COMPOSE_DEADLINE` (120 s); `ROUTER_BUDGET` plus three… | APP-096, APP-097 |
| DS-25 | Debt (review m1) | Free-text members (titles, descriptions, intent) are admitted unscanned; renderers are the only defense against markup in model-written text. | APP-097 |
| DS-26 | Debt (review m2) | `runnable_view` accepts a data view over a `records` input that the query outline never lists, so a generated view can be published and show nothing. | APP-097 |
| DS-27 | Debt (review m3) | Compose is rate limited per member only (6 per hour); no organization or service window; limiter is in-process and resets on restart. | APP-097, APP-061 |
| DS-28 | Debt (review m4) | Engine literal parsing: `int()` accepts non-ASCII digits and raises `ValueError` as `failed/engine` instead of `refused`; unreachable through admitted… | APP-097 |
| DS-29 | Debt (review m5) | No route-level test for `GET /v1/library` (anonymous, wrong method, store outage). | APP-097 |
| DS-30 | Debt (review m7) | `seed-library` is fixture-organization-only (`ORGANIZATIONS` allowlist); the beta organization must be one of them or the allowlist extended deliberately. | APP-096, APP-097 |
| DS-31 | Debt | Library sources are labeled synthetic CSVs generated by `make_library.py`; the router sidecar `catalog.json` is compiled in, so a new library app needs a… | APP-097 (PH03) |
| DS-32 | Debt | Spend Review `expenses` and `queue` views fail honestly on the local stand (its expenses revision is not in the engine cache) and `status` is denied; the… | APP-096, APP-097 |
| DS-33 | Debt | Generated views restricted to the engine's qualified subset (one input; filter, project, aggregate, time_bucket, sort, limit) and edition 1 content (no… | APP-097 (PH02 Track C) |
| DS-34 | Debt | Service Dockerfile contracts: the service image copies `query_engine.py`, runtime and admin sources; every new pitch file the service needs at run time… | APP-096, APP-089, APP-097 |
| DS-35 | Debt | Preflight supersede mechanism (`supersedes_window_id`) added for the pitch redeploy; the earlier window's figures are kept and a later month counts zero. | APP-096, APP-086, APP-097 |
| DS-36 | Debt | Pitch relaxations of ADR-010 inherited from ADR-011 section 4: model call before APP-043, client islands beyond APP-003, review depth risk-tiered. | APP-097 |
| DS-37 | Debt | the composer authenticates to the Anthropic API with a static API key (a dedicated `genui-pitch` key in the `ComposerModel` secret, injected only into the… | APP-097 |
| DS-38 | Shortcut | In a demo organization, rehearsed intents are answered from the library catalog without a model call (word match against titles/descriptions). | APP-097 |
| DS-39 | Shortcut | The demo organization is the Canary fixture tenant `…0002` (`GENUI_DEMO_ORGANIZATIONS`). | APP-096, APP-097 |
| DS-40 | Shortcut | The canvas synthetic-data label and fixed demo layout are keyed off the demo organization set. | APP-097 |
| DS-41 | Shortcut | `grant-library-access` stands in for persona provisioning: one operator command gives a member the whole library and an authoring app. | APP-097 |
| DS-42 | Debt | `seed-library` is limited to the fixture organizations allowlist. | APP-096, APP-097 |
| DS-43 | Planned shortcut | Rehearsed utterances parse deterministically inside a listed demo organization: the four sentences of `docs/pitch/SCENARIOS.md` (file an expense with an… | APP-097, APP-043 |
| DS-44 | Planned shortcut | Member display names come from an operator-maintained file (`GENUI_DISPLAY_NAMES_FILE`, `{"members":[{"principal_id","display_name"}]}`) read at startup;… | APP-097 |
| DS-45 | Planned shortcut | Pre-existing expense claims for the demo are filed through the real action and command paths before the pitch (the campaign and the stand do it: one… | APP-096, APP-097 |
| DS-46 | Debt | The action path's `confirm_token` is a random correlation id in the log and the card; it grants nothing and the command route re-judges the posted draft on… | APP-097 |
| DS-47 | Debt | The expense claims app's data views (claims to date, past claims by status) read the synthetic `expense_claims` source, not the live records (Phase 1 runs… | APP-097 |
| DS-48 | Planned shortcut | MCP clients authenticate with a member-minted bearer agent token, not OAuth 2.1; consumer connector UIs (claude.ai custom connectors, ChatGPT developer-mode… | APP-097 |
| DS-49 | Debt | MCP and Slack drafts are held in process memory (10-minute TTL, single use); a restart or a second task loses pending drafts. | APP-097, APP-061 |
| DS-50 | Planned shortcut | Slack is installed to one workspace by hand; the bot token and signing secret are deployment secrets, not per-workspace OAuth v2 installs. | APP-097 |
| DS-51 | Planned shortcut | Demo personas' Slack links are made once before the pitch through the real `/forma link` flow by the people holding the demo accounts; there is no admin… | APP-097 |
| DS-52 | Debt | Slack handlers finish in a detached `tokio::spawn` after the ack, with no queue or retry; a crash after the ack loses the reply (the record, if committed,… | APP-097, APP-061 |
| DS-53 | Planned shortcut | Inside a demo organization the rehearsed transform sentences ("Review expenses over $200", "Investigate outbound traffic from build-worker-17", "Show… | APP-101, APP-097 |
| DS-54 | Debt | The adapted ⇄ standard lineage (a share of the standard work also shares its adaptation, so a teammate reshapes live) is kept in process memory, bounded to… | APP-097, APP-061 |
| DS-55 | Debt | "Save as /name" creates a new saved work of the same publication and bindings whose `context` carries the slug (`forma-command:/<slug>`), because… | APP-097 |
| DS-56 | Shortcut | Receipt images are parsed and discarded; the record notes `receipt: attached` and the card shows a client-side thumbnail, but no image is stored. | APP-097 |
| DS-57 | Shortcut | Inside the demo organization the rehearsed receipt fixture (`docs/pitch/receipt-fixture.png`, synthetic store "Greenleaf Bowl Co.", matched by SHA-256)… | APP-097 |
| DS-58 | Debt | The expense ledger aggregates client-side over the newest 200 live records; older records are not counted. | APP-097 |
| DS-59 | Debt | The canvas's "via Slack" origin chip comes from an in-process map of the record versions Slack committed (`crate::origins`), not from the audit chain; a… | APP-097, APP-100 |
| DS-60 | Debt | A Slack link binds the Slack user who ran `/forma link` to whichever member opens the URL while signed in and confirms; nothing checks that the Slack… | APP-097, APP-100 |
| DS-61 | Debt | "The approver" for the Slack DM is derived, not stored: every linked member who now holds `record_write` on the work, did not file the record and finds it… | APP-097, APP-100 |
| DS-65 | Planned shortcut | The weather widget serves fixture data inside the demo organization (and whenever Open-Meteo is unreachable), labeled. | APP-097 |
| DS-66 | Planned shortcut | Widget customization is a closed set of skins, palettes and fonts; rehearsed sentences map to fixed states without a model call in the demo organization. | APP-097 |
| DS-67 | Debt | The set of cards a member brought onto the canvas (the library pane, `/card`, `/app`, compositions and placed apps) is kept in this browser's localStorage… | APP-097 |
| DS-68 | Debt | No canvas performance regression harness: the earlier canvas's pan/zoom/drag event-cost and settle budgets… | APP-097 |
| DS-69 | Planned shortcut | Inside a listed demo organization the rehearsed workflow sentences of `docs/pitch/SCENARIOS.md` ("Workflows") parse without a model call: time off ("Book me… | APP-097, APP-107 |
| DS-70 | Planned shortcut | The cross-vendor act's second vendor is Claude Haiku on the local stand, not OpenAI: `app/scripts/mcp_haiku_agent.py` gives Haiku exactly the tools… | APP-099, APP-107, APP-097 |
| DS-71 | Debt | Approval steps are not bound to roles: any member who holds `record_write` on the shared work and did not file the record may take any transition the… | APP-097, APP-107 |
| DS-72 | Debt | The PO number of a purchase request is derived, not stored: `PO-` and the first eight hex digits of its record id; the invoice's PO match is computed per… | APP-097, APP-107 |
| DS-73 | Planned shortcut | Inside a listed demo organization a folder build whose purpose mentions metrics, KPIs, a dashboard or an overview ("Build an app that shows the key metrics… | APP-097, APP-108 |
| DS-74 | Planned shortcut | Inside a listed demo organization the explanation card of a folder-built app is worded by rule from its KPI views ("<file> holds {rows} rows; total revenue… | APP-097, APP-108 |
| DS-75 | Planned shortcut | Inside a listed demo organization a folder's brief is rehearsed: data paragraphs worded by rule from each data card's KPIs (placeholders filled from… | APP-097, APP-108 |
| DS-76 | Debt | In a folder-built app's model context PDFs, images and text files contribute their names only (the brief reads small PDFs with citations; the build does… | APP-097, APP-108 |
