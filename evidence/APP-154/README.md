# APP-154 private live sharing qualification

Date: 2026-11-01. Source branch: `codex/app-sharing-2026-11-01`, based on `pivot/pitch-mvp` commit `1b2f21f4cbab2b3c5fa4aa663ca40c958d492ee4`. The landing PR and its exact-head checks bind the final source. These are local source qualifications, not a hosted release receipt.

## Result

Named, private, live, view-only sharing is implemented under [ADR-014](../../docs/architecture/ADR-014-LIVE-APP-SHARING.md). The recipient verifies the named mailbox and explicitly accepts one app. No organization membership, entitlement or source grant is copied. Canvas placement is separately permitted and explicitly requested. Connected pages/cards continue to read the original persisted work, with request-scoped guest authority and receipts. Definition/source-binding changes pause the share; record changes remain live.

The owner can revoke view access, remove placement permission independently, and manage each independent grant. Shared-account signout preserves an existing home workspace. Forwarded links, wrong identities, expired sessions, stale grant versions, sponsor permission loss and changed publication policy do not release app data. The new signup capability is confined to a separate tenant-pinned operator broker; it is not a management key in the normal application.

## Evidence

| Check | Result | Retained evidence |
| --- | --- | --- |
| Native sharing storage and production HTTP, real PostgreSQL and confined query engine | 13 grouped checks passed | [Campaign](live-sharing-campaign.json), [commands and source hashes](storage-validation.json) |
| OAuth callback and account lifecycle, real PostgreSQL with local HTTP provider/broker contract fakes | 6 grouped checks passed | [Identity campaign](shared-identity-campaign.json) |
| Migration, catalog, roles, RLS and lock qualification | 381 checks passed; historical logical catalogs 1–16 unchanged | [Schema qualification](sharing-schema-qualification.json), [pins](sharing-schema-pins.json) |
| Actual browser product journey | 8 grouped checks passed; no page errors; mobile has no page overflow | [Browser evidence](browser/README.md) |
| Independent authority/identity/storage/broker review | Findings fixed and rechecked | [Review and dispositions](REVIEW.md) |

The browser used a genuinely generated and persisted Reading List, production owner/share routes and real audited record commands on the disposable dev PostgreSQL stand. The identity listener injected explicitly synthetic verified accounts; it did not stand in as evidence of hosted signup. A record appeared on the shared document after **1,592 ms** and its connected canvas after **2,374 ms**. Revocation cleared the document after **551 ms** and the canvas after **1,112 ms**. These are observed local timings, not a production performance guarantee. Tests also cover offline/reconnect, inventory outages preserving layout, wrong-account refusal, exact retry after a lost create response, explicit existing-member placement and separate signout.

`just check` ran all repository steps. The initial Rust test pass exposed old contract-edition/domain/fixture assertions, missing managed source-inventory entries and native-page test helpers that had not bound the two new owners; they were corrected and the affected steps rerun. The initial non-Rust run also exposed the catalog guard's old byte-bound expectation, corrected for additive schema 17. A migration-under-edit qualification was discarded and repeated against the stable catalog; the retained 381-check report is the final one. All check steps passed after targeted fixes. Final command results are recorded in [checks.json](checks.json) and the landing PR: 352 library tests, 7 native SSR tests, 3 compile-fail doctests, 74 frontend tests and 16 isolated broker tests. The six skipped operator-package tests require a sealed image/parser environment and are not presented as passing image qualification.

## Limits and remaining trial work

- No AWS migration, IAM change, image push, provider administration or hosted deployment was performed. Existing owner instances on ports 4001–4003 and 4010 were preserved; only this worktree's disposable 7601–7604 stand was used and stopped.
- [APP-139](../../Tickets/0-backlog/APP-139.md) must configure/deploy the isolated broker, dedicated durable secret, private HTTPS route and exact `/auth/shared/callback` redirect, then prove actual provider signup, mailbox verification and return to the app. [APP-140](../../Tickets/0-backlog/APP-140.md) owns the recording. See the [operations runbook](../../docs/operations/LIVE-SHARING.md). The broker's single-writer, byte and durable write budgets are explicit MVP bounds.
- Shared content is all rendered published views/records. A temporary client filter does not narrow the grant. No anonymous link mode, static export, recipient edit/reshare/raw-source access or automatic organization admission is added.
- Browser testing found an unrelated owner create-form choice issue: a generated command with one declared initial enum still offers other states, which storage correctly denies. It is recorded as an [APP-144](../../Tickets/4-done/APP-144.md) follow-up; the sharing journey used the admitted initial state without weakening permissions.
- APP-154's source can land while historical predecessor/phase gates remain open. Its board status does not claim those gates or the hosted first-run trial passed.
