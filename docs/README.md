# Docs

Every Markdown file under `docs/` is in the table below. Read this index before the rest of the tree.

**Current** is a maintained procedure or the live direction. **Reference** is an ADR or other decision record still in force, or a design brief. **Historical** is a dated pitch-era note, a superseded plan, a rehearsal or pitch script, or a dated audit. The pitch ended on 2026-11-04. `main` is the trunk ([ADR-019](architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)).

| Doc | Status | Use it for | Superseded by |
| --- | --- | --- | --- |
| [AGENTS.md](AGENTS.md) | Current | How to work in this folder, and which check covers a docs change. | |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Current | Setup, local instances, checks and the path to beta. | |
| [README.md](README.md) | Current | This index. | |
| [architecture/ADR-001-RELEASED-BASELINE.md](architecture/ADR-001-RELEASED-BASELINE.md) | Reference | The published Forma 3.2.0 pin this app uses. | |
| [architecture/ADR-002-CONTRACT-EDITION.md](architecture/ADR-002-CONTRACT-EDITION.md) | Reference | Contract-edition rules and the five-second revocation bound. | |
| [architecture/ADR-003-PUBLIC-BETA-ACCESS.md](architecture/ADR-003-PUBLIC-BETA-ACCESS.md) | Reference | The public beta origin and invite-first admission. | |
| [architecture/ADR-004-TENANT-TRANSACTIONS.md](architecture/ADR-004-TENANT-TRANSACTIONS.md) | Reference | Tenant transactions, pool ownership and the foundation slices. | |
| [architecture/ADR-005-PHASE-1-SEQUENCING.md](architecture/ADR-005-PHASE-1-SEQUENCING.md) | Reference | How Phase 1 split local proof from hosted proof. | |
| [architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md](architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) | Reference | Sign-in, the verification-pending state and resend. | |
| [architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md](architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) | Reference | Phase 1 write authority, schema layout and query execution. | |
| [architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md](architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md) | Reference | The YC demo plus an invite-first beta, and which hosted checks moved later. | |
| [architecture/ADR-009-PHASE-2-DATA-PLANE.md](architecture/ADR-009-PHASE-2-DATA-PLANE.md) | Reference | The Phase 2 data plane, job authority and workspace presentation. Read it with its amendment record below. | |
| [architecture/ADR-009-AMENDMENTS.md](architecture/ADR-009-AMENDMENTS.md) | Reference | Dated amendments to ADR-009, kept outside the ADR because the contract lock hashes it: the job queue and worker authority fixes and the edition-two decisions (APP-088). | |
| [architecture/ADR-010-NOV-2-MVP-CUT.md](architecture/ADR-010-NOV-2-MVP-CUT.md) | Reference | The November 2 MVP cut. ADR-011 superseded only its MVP definition for that date. | |
| [architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md](architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md) | Reference | The pitch pivot and the later clean-trial amendment. | |
| [architecture/ADR-012-DEVELOPER-WORKFLOW.md](architecture/ADR-012-DEVELOPER-WORKFLOW.md) | Reference | One dev profile, one command front door and deploy recipes in Git. | |
| [architecture/ADR-013-REPOSITORY-AND-DELIVERY.md](architecture/ADR-013-REPOSITORY-AND-DELIVERY.md) | Reference | One clone, the root workspace and the delivery ladder. ADR-019 replaces the freeze of `main`. | |
| [architecture/ADR-014-LIVE-APP-SHARING.md](architecture/ADR-014-LIVE-APP-SHARING.md) | Reference | Named live app sharing without making the recipient an organization member. | |
| [architecture/ADR-015-COMPOSED-APP-PRESENTATION.md](architecture/ADR-015-COMPOSED-APP-PRESENTATION.md) | Reference | One composed app across canvas, page and shared view. | |
| [architecture/ADR-016-WORKSPACE-SIGNIN.md](architecture/ADR-016-WORKSPACE-SIGNIN.md) | Reference | Workspace entry and verified workspace discovery. | |
| [architecture/ADR-017-BETA-PROMOTION.md](architecture/ADR-017-BETA-PROMOTION.md) | Reference | One-approval promotion. ADR-019 moves the deploy branch to `main`. | |
| [architecture/ADR-018-WHOLE-APP-VIEWS.md](architecture/ADR-018-WHOLE-APP-VIEWS.md) | Reference | An app is one object. Card, stack and open are views. | |
| [architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md](architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md) | Reference | `main` is the trunk, and the reservation ceiling is $180 / $218. | |
| [architecture/CANVAS-WINDOWS.md](architecture/CANVAS-WINDOWS.md) | Reference | The app-window shell and file cards decided on 2026-11-02. | |
| [architecture/CSV-APP-DATA.md](architecture/CSV-APP-DATA.md) | Reference | Editable data in an app built from a spreadsheet. | |
| [architecture/DECISIONS.md](architecture/DECISIONS.md) | Reference | Implementation decisions from 2026-10-28. Product order is DIRECTION.md. | |
| [architecture/FORMA-BRAND.md](architecture/FORMA-BRAND.md) | Current | The platform visual identity. | |
| [architecture/FRAMEWORK-BASELINE.md](architecture/FRAMEWORK-BASELINE.md) | Reference | Released Forma 3.2 behavior this repository binds. | |
| [architecture/MODULE-MAP.md](architecture/MODULE-MAP.md) | Current | Rust module layers, allowed dependencies and known leaks. | |
| [architecture/PITCH-JOIN.md](architecture/PITCH-JOIN.md) | Historical | The APP-091 seam note for canvas, matcher and composer, read against an old pitch commit. | [MODULE-MAP.md](architecture/MODULE-MAP.md) |
| [architecture/SPREADSHEET-VIEWING.md](architecture/SPREADSHEET-VIEWING.md) | Reference | Limits for viewing a large spreadsheet. | |
| [architecture/THEME-ROLES.md](architecture/THEME-ROLES.md) | Reference | Shared color roles for the canvas, native pages, apps and widgets. | |
| [design-brief/README.md](design-brief/README.md) | Reference | Designer brief dated 2026-10-30 for the invite-first beta screens. | |
| [operations/LIVE-SHARING.md](operations/LIVE-SHARING.md) | Current | Private live sharing and the separate invitation broker. | |
| [pitch/README.md](pitch/README.md) | Current | Why `docs/pitch/` still holds live procedures, and which file is which. | |
| [pitch/BETA-PROMOTION.md](pitch/BETA-PROMOTION.md) | Current | The one-approval path from a merge on `main` to beta. | |
| [pitch/CANVAS-RULES.md](pitch/CANVAS-RULES.md) | Reference | Canvas grammar. ADR-018 and CANVAS-WINDOWS override the older component-placement rules. | |
| [pitch/CLEAN-TRIAL-DEPLOY.md](pitch/CLEAN-TRIAL-DEPLOY.md) | Current | Clean-profile operator procedure. Its November 2 status block is behind the sealed release. | |
| [pitch/CONNECTIONS.md](pitch/CONNECTIONS.md) | Reference | How a person, an agent and the model each connect. Its definitions point to pitch/GUIDE.md, which is Historical. | |
| [pitch/DEPLOY-RUNBOOK.md](pitch/DEPLOY-RUNBOOK.md) | Current | Operator steps for schema and infrastructure. Laptop mutation recipes are blocked after the promotion cutover. | |
| [pitch/GUIDE.md](pitch/GUIDE.md) | Historical | Pitch-build tour, including the Northstar sign-in alias and the demo highlights. | [DEVELOPMENT.md](DEVELOPMENT.md) |
| [pitch/PITCH-SCRIPT.md](pitch/PITCH-SCRIPT.md) | Historical | Spoken script for the investor demo. | [planning/first-run/RECORDING.md](planning/first-run/RECORDING.md) |
| [pitch/RECORDING.md](pitch/RECORDING.md) | Historical | Shot list for the seeded YC demo. | [planning/first-run/RECORDING.md](planning/first-run/RECORDING.md) |
| [pitch/REHEARSAL-2026-10-31.md](pitch/REHEARSAL-2026-10-31.md) | Historical | Rehearsal log for the 2026-10-31 pitch script pass. | |
| [pitch/RELEASES.md](pitch/RELEASES.md) | Current | Beta release log. Operator code appends a row here. | |
| [pitch/SCENARIOS.md](pitch/SCENARIOS.md) | Historical | Seeded two-window choreography for the concept-car rehearsal. | [planning/first-run/README.md](planning/first-run/README.md) |
| [pitch/SLACK-SETUP.md](pitch/SLACK-SETUP.md) | Current | Slack app secrets for local development and beta. The checkout path in the steps is the old clone name. | |
| [planning/APP-129-GAP-MATRIX.md](planning/APP-129-GAP-MATRIX.md) | Historical | Contract-gap read against one pitch commit. | |
| [planning/APP-135-CLEAN-CUSTOMER-AUDIT.md](planning/APP-135-CLEAN-CUSTOMER-AUDIT.md) | Historical | Dated clean-customer audit. | |
| [planning/APP-135-FILES-FOLDERS-AUDIT.md](planning/APP-135-FILES-FOLDERS-AUDIT.md) | Historical | Dated files-and-folders audit. | |
| [planning/CANVAS-PRODUCT-SHOWCASE.md](planning/CANVAS-PRODUCT-SHOWCASE.md) | Historical | How to replay the local Spend and Jacket showcase branch. | |
| [planning/COMPOSED-APP-IMPLEMENTATION-2026-11-01.md](planning/COMPOSED-APP-IMPLEMENTATION-2026-11-01.md) | Historical | Implementation record for the composed-app release `9ee4a01`. | |
| [planning/CONTINUATION-2026-10-31.md](planning/CONTINUATION-2026-10-31.md) | Historical | October 31 continuation. It still calls `pivot/pitch-mvp` the trunk. | [architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md](architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md) |
| [planning/CROSS-TEAM-HANDOFF.md](planning/CROSS-TEAM-HANDOFF.md) | Current | Which team owns framework and fAuth gates. Statuses inside the table are not the live board. | |
| [planning/DECISIONS-AND-RISKS.md](planning/DECISIONS-AND-RISKS.md) | Historical | October preparation risks attached to upstream gates. | [planning/CROSS-TEAM-HANDOFF.md](planning/CROSS-TEAM-HANDOFF.md) |
| [planning/DEFERRED.md](planning/DEFERRED.md) | Reference | Candidate journeys that are not tickets until their trigger is met. | |
| [planning/DEV-WORKFLOW-INVENTORY.md](planning/DEV-WORKFLOW-INVENTORY.md) | Historical | Measured workflow inventory that ADR-012 decided. | [DEVELOPMENT.md](DEVELOPMENT.md) |
| [planning/DEVELOPMENT-DRY-RUN-2026-10-31.md](planning/DEVELOPMENT-DRY-RUN-2026-10-31.md) | Historical | Dry run of the developer guide on 2026-10-31. | |
| [planning/DIRECTION.md](planning/DIRECTION.md) | Current | The November 1 product direction and its ticket order (APP-145 onward). The current next slices are named in the root README (APP-163 to APP-165). | |
| [planning/EXISTING-WORK.md](planning/EXISTING-WORK.md) | Historical | Audit of nearby repositories on 2026-10-28. | |
| [planning/FOUNDER-NOTES-AUDIT-2026-10-31.md](planning/FOUNDER-NOTES-AUDIT-2026-10-31.md) | Historical | Audit of the approved handoff moves on 2026-10-31. | |
| [planning/LATER.md](planning/LATER.md) | Current | Work that stays behind the current slices. | |
| [planning/MVP-GAPS-2026-11-01.md](planning/MVP-GAPS-2026-11-01.md) | Historical | Gap-completion record from 2026-11-01, behind later releases. | |
| [planning/MVP-NOV-2-PLAN.md](planning/MVP-NOV-2-PLAN.md) | Historical | Day-by-day plan for the November 2 cut. | [planning/DIRECTION.md](planning/DIRECTION.md) |
| [planning/PHASE-1-EXECUTION-PLAN.md](planning/PHASE-1-EXECUTION-PLAN.md) | Historical | Execution checklist from 2026-10-29. It says nothing in it was implemented yet. | [planning/DIRECTION.md](planning/DIRECTION.md) |
| [planning/PHASE-2-EXECUTION-PLAN.md](planning/PHASE-2-EXECUTION-PLAN.md) | Historical | Phase 2 task plan. Phases 2–6 are not the current queue. | [planning/DIRECTION.md](planning/DIRECTION.md) |
| [planning/PHASE-2-HANDOFF.md](planning/PHASE-2-HANDOFF.md) | Historical | Transfer note from 2026-10-29. The ticket statuses in it are stale. | |
| [planning/PIVOT-REVIEW-2026-11-01.md](planning/PIVOT-REVIEW-2026-11-01.md) | Historical | Review snapshot from 2026-11-01. | [planning/DIRECTION.md](planning/DIRECTION.md) |
| [planning/QUALITY-GATES.md](planning/QUALITY-GATES.md) | Current | Phase checks, trials and what counts as evidence. | |
| [planning/ROADMAP.md](planning/ROADMAP.md) | Current | Phase map. Current work is DIRECTION.md, not the old first-ticket queue. | |
| [planning/SOURCE-GUIDE.md](planning/SOURCE-GUIDE.md) | Current | Reading order for the frozen design and which later text outranks it. Some sentences describe the repository before the application existed. | |
| [planning/START-HERE.md](planning/START-HERE.md) | Current | Pointer to AGENTS.md, the README, DEVELOPMENT.md and this index. | |
| [planning/SYNTHETIC-ENVIRONMENT-COST.md](planning/SYNTHETIC-ENVIRONMENT-COST.md) | Historical | APP-007 cost estimate, including the retired $100 plan. | [architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md](architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md) |
| [planning/SYNTHETIC-ENVIRONMENT.md](planning/SYNTHETIC-ENVIRONMENT.md) | Historical | APP-007 operating record. Its current-operator section is the 2026-10-28 $100 decision. | [architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md](architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md) |
| [planning/app-model/app.md](planning/app-model/app.md) | Reference | What an app is. Sentences marked proposed are not in the code yet. | |
| [planning/app-model/box.md](planning/app-model/box.md) | Reference | A box is the folder widget. Formation can produce an app. | |
| [planning/app-model/components.md](planning/app-model/components.md) | Reference | A component is one part of an admitted app, on the board and on the page. | |
| [planning/app-model/composed-app.md](planning/app-model/composed-app.md) | Reference | The composed-app experience decided from the 2026-11-01 screenshots. | |
| [planning/app-model/library-pane.md](planning/app-model/library-pane.md) | Reference | The four library tabs stay, with contextual Ask. | |
| [planning/app-model/widget.md](planning/app-model/widget.md) | Reference | What a widget is, and which kinds exist. | |
| [planning/first-run/README.md](planning/first-run/README.md) | Current | The clean first-run trial. The seeded choreography is not that trial. | |
| [planning/first-run/RECORDING.md](planning/first-run/RECORDING.md) | Current | Capture plan for the clean trial. It is not a completed rehearsal. | |
