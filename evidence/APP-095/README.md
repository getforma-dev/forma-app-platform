# APP-095 — share a work with a teammate; real-time updates (ADR-011 pitch, `pivot/realtime`)

Date 2026-10-30. Base `9cf52c2` (`pivot/pitch-mvp`), sub-branch `pivot/realtime` of the clone `FormaBuiltApps/forma-pitch-mvp`, not merged. Status: implemented and qualified **locally** on disposable PostgreSQL 16.13 in Docker with verified TLS. No hosted run, no browser, no managed (RDS) run, no CI run, no independent review; the ticket stays Backlog on the board (its predecessor APP-094 is not Done). Controlling text: [APP-095](../../Tickets/0-backlog/APP-095.md); [ADR-011](../../docs/architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md) §3 invariants I1, I2, I5 and the 2026-10-30 amendment ("sharing with a teammate under the sharer's own permissions; real-time updates (a server-sent event stream re-authorized at delivery)"); [ADR-007](../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) D7.1, D7.3, D7.8, D7.9, D7.12, D7.14, D7.15, D7.18 and the 2026-10-29 creator-authority amendment; [ADR-006](../../docs/architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) for the session that gates both routes. Where this slice departs from ADR-007 the departure is recorded as debt (below and in ADR-011 §Consequences, APP-097).

## What changed

| Area | Change |
| --- | --- |
| Migration 0008 | `app/migrations/0008_sharing.sql` (sha256 `c98ede67…405b`). Both version CHECKs `BETWEEN 1 AND 8`. One definer function `genui_private.share_work(p_org, p_sharer, p_grantee, p_app, p_work, p_record_write) RETURNS TABLE(scope, resource_id, permission, inserted)`, `SECURITY DEFINER STRICT VOLATILE`, `search_path = pg_catalog, pg_temp`, owned by `genui_admit`, `PUBLIC` revoked, EXECUTE to `genui_workspace`. It requires the transaction scope to be the sharer's own organization, principal and current epoch (as `lock_member_authority`), takes `organizations FOR SHARE` and the sharer's membership `FOR SHARE`, requires the grantee to be an active member under the same scope (RLS), requires the sharer to be the work's unrevoked creator or to hold `share_work` on it and `run` on the app, and copies onto the grantee exactly `discover`/`run` on the app that the sharer holds, `record_read` on the work, `record_write` only when asked for and held, and `source_read` on each source dependency the sharer holds (`ON CONFLICT DO NOTHING`; `inserted` says what was new). It takes no permission names and exposes no bypass. When a row was inserted it appends one `permission.granted` event on the admission chain (actor the sharer, `app_id`, `work_id`, `outcome_digest` = `sha256(forma:share-grant:v1 ‖ 0 ‖ grantee ‖ 0 ‖ granted list)`). A4 extension: `genui_admit` gains column SELECT on `memberships.status`, `saved_work(organization_id, app_id, work_id, created_by, creator_revoked_at)`, `work_source_dependencies(organization_id, app_id, work_id, source_id)` and the three permission tables; the `admission_chain` INSERT policy is widened in place with `permission.granted`. |
| Catalog and pins | `catalog-v8.json` generated (sha256 `8c97dc4e…62dd`; body 283,640 B, file 278,118 B, guard 419,430); v1–v7 regenerate byte-identically ([generation.json](generation.json)). Every pin at 8 ([schema-pins.json](schema-pins.json), `agree: true`): `manifest.json`, `runtime.py DATABASE_SCHEMA_VERSION`, `catalog.rs`, `managed_qualification.rs`, `pool.rs verify_schema_version`; the native campaign's unsupported/restored literals moved to 9/8. `.dockerignore`, the operator and qualification `Dockerfile.dockerignore` list 0008, `catalog-v8.json` and the new sources; `managed_qualification.rs SOURCES` 87 (was 81). |
| Storage | `app/src/storage/sharing.rs` (new): `Workspace::members` (`fence_member`, the other active members, neutral labels `Member <8 hex>`), `Workspace::share_work` (precheck `record_read` on the work → `fence` → `record_write` re-read when asked → the definer; no rows is `Denied`), `Data::can_open` (the stream's delivery check: `discover` on the app and `record_read` on the work, as `openable_works`), `Data::member_current` (`fence_member`: the stream's liveness check). `authorization.rs`: read-only `TrustedRequest::principal()`. `mod.rs` exports `Granted`, `Member`, `Shared`, `member_label`, `SHARE_HANDLER_DEADLINE`. |
| Routes | `app/src/sharing/mod.rs` (new): `GET /v1/members`, `POST /v1/work/{app}/{work}/share` (JSON ≤ 64 KiB, closed body, fetch-metadata rule, uniform 404, 400 `malformed_share`). `app/src/events/` (new): the in-process hub (`tokio::sync::broadcast`, `hub()`), `GET /v1/events` (SSE; session-gated; one stream per session; ≤ 4 per member, ≤ 256 in the process, 429 past them; heartbeat 15 s; re-check 30 s; lifetime 1 h), the per-connection pump that re-authorizes every event at delivery, `Hub::app_placed` for the composer. `commands/routes.rs`: one hook after `CommandResult::Committed` (`hub().record_committed`). `main.rs` merges both routers. `Cargo.toml`: `futures-core = "=0.3.34"` (already in the lock through axum/hyper; the SSE body stream type). |
| Qualifier | `qualify_tenant_schema.py`: `ADMIT_FUNCTIONS` + `share_work`, `ADMIT_GRANTS` + the 0008 reads, `CHAIN_KINDS['genui_admit']` + `permission.granted`, the boolean probe literal. |
| Campaign | `app/src/storage/sharing_qualification.rs` (new, `#[ignore]`): `native_sharing_campaign`, 12 checks ([sharing-campaign.json](sharing-campaign.json)). `app/src/events/tests.rs`: hub bounds, replacement, pump verdicts, timestamps, the 404 without a session; `sharing::tests`: the closed body and the refusals. |

## Contracts

Share: `POST /v1/work/{app}/{work}/share`, `Content-Type: application/json`, `Sec-Fetch-Site: same-origin` (or the exact `Origin`), the workspace session cookie; body `{"grantee_principal_id":"<uuid>","record_write":false}` (`record_write` optional, default false; no other member). 200 `{"app_id":"<uuid>","work_id":"<uuid>","grantee_principal_id":"<uuid>","granted":[{"scope":"app"|"work"|"source","resource_id":"<uuid>","permission":"discover"|"run"|"record_read"|"record_write"|"source_read","inserted":true|false}, …]}` (forma-json-v1, `no-store`). 403 `cross_site`; 404 `not_found` (uniform: no or ended session, a work the sharer cannot open, no authority to share, a `record_write` the sharer does not hold, a grantee who is the sharer, revoked, unknown or of another organization, a nonexistent work); 413 `payload_too_large`; 400 `malformed_share`; 503 `unavailable`. A share that inserted a row announces `app.shared`.

Members: `GET /v1/members` → 200 `{"members":[{"principal_id":"<uuid>","label":"Member <8 hex>"}, …]}` (the other active members of the caller's organization, at most 100; labels only), else the uniform 404.

Events: `GET /v1/events` with the session cookie → 200 `text/event-stream`, `Cache-Control: no-store`; frames `event: record.committed|app.shared|app.placed` / `data: {"kind":"<same>","app_id":"<uuid>","work_id":"<uuid>","at":"YYYY-MM-DDTHH:MM:SSZ"}`; comment heartbeat `: heartbeat` every 15 s. The uniform 404 without a live session; 429 `too_many_requests` (`Retry-After: 2`) past the bounds. Every event is delivered only after `Data::can_open` passes for that subscriber at that moment; an event of another organization is dropped before any lookup; a refusal re-checks `Data::member_current`, and a member who is no longer current (sign-out, operator revocation: any epoch change; lapsed entitlement) ends the stream. The stream ends after one hour; the client reconnects.

Composer hook: `genui::events::hub().app_placed(organization, app, work)` after the saved work exists.

## Authorization rules as implemented

1. Sharer: a live workspace session (`resolve_session`), current membership at its epoch with a current entitlement, `record_read` on the work by the standard clause (unrevoked creator or `record_read` grant, `run` on the app, live `source_read` on every dependency), the `genui_lock` fence, then inside the definer: unrevoked creator **or** `share_work` on the work, and `run` on the app. `record_write` is shared only when requested and held (re-read after the fence and in the definer).
2. Grantee: an active membership in the **same** organization (the trusted request's; RLS scope), not the sharer.
3. Granted: the intersection of {`discover`, `run` on the app; `record_read`, optionally `record_write` on the work; `source_read` on the work's source dependencies} with what the sharer holds now. Never `edit_definition`, `review_publish`, `share_work`, `export`, `admin_policy`; never a row in another organization. A grantee cannot pass grants on (no `share_work`, not the creator). Repeat shares write nothing and append no event.
4. Delivery: an event reaches a subscriber only if that subscriber could open the work at that moment (`discover` on the app and `record_read` on the work, the listing clause); ids of works a member cannot open never reach the wire.

## Exact commands and results

`PYC` = `/private/tmp/claude-501/-Users-victorvillacis-dev/df0e31da-4f04-46a7-8043-30cdac91f9c2/scratchpad/app013-merge/op-python` (the main checkout's `app/runtime/.venv` python with the contracts venv on `PYTHONPATH`); `app/runtime/.venv`, `contracts/.venv` and `app/admin/dist` are symlinks to the main checkout in this worktree (not committed). `cargo +1.93.1`; one Docker campaign at a time; every container the fixtures started was removed.

| Command (repository root unless noted) | Result |
| --- | --- |
| in `app/runtime`: `PYC generate_schema_catalog.py --output-dir <scratch>` | `container_removed: true`; v1–v7 identical to the pinned files; v8 adopted ([generation.json](generation.json)) |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`, every pin 8 ([schema-pins.json](schema-pins.json)) |
| in `app/`: `cargo +1.93.1 fmt --check`; `cargo +1.93.1 clippy --locked --all-targets -- -D warnings`, and with `--features qualification-harness` | exit 0 all three |
| in `app/`: `cargo +1.93.1 test --locked` | lib **141 passed / 14 ignored**, bin 5, doctests 3 (new: `events::tests` ×8, `sharing::tests` ×2, `storage::sharing::tests` ×1) |
| in `app/runtime`: `PYC qualify_tenant_schema.py --output <scratch>` | `passed: true`, **362 checks** (was 359; the seventh definer, its grants and the widened admission chain asserted), container removed ([schema-campaign.json](schema-campaign.json)) |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PYC GENUI_SHARING_REPORT=<scratch> cargo +1.93.1 test --locked --lib storage::sharing_qualification::native_sharing_campaign -- --ignored --exact --nocapture` | 1 passed, **12 checks** ([sharing-campaign.json](sharing-campaign.json)). Earlier runs fixed test expectations only: the seeded approver's `review_publish` is on the Northstar fixture app; the seeded actors hold Northstar fixture rows (the permission listing is now scoped to the S05 resources); an operator revocation advances the member's epoch (the campaign now re-derives the request, and expects the stream to **close** on that epoch change, as the ticket asks); the seeded actors exist in both organizations |
| `… storage::command_qualification::native_command_campaign …` | 1 passed, 21 checks ([command-campaign.json](command-campaign.json)) |
| `… auth::tests::native_session_campaign …` | 1 passed, 20 checks ([session-campaign.json](session-campaign.json)) |
| `… storage::qualification::native_postgres_campaign …` | 1 passed, 36 checks: the embedded `catalog-v8.json` matches the migrated database ([native-campaign.json](native-campaign.json)) |
| `… storage::admission_qualification::native_admission_campaign …` | 1 passed, 27 checks ([admission-campaign.json](admission-campaign.json)) |
| `… storage::definition_qualification::native_definition_campaign …` | 1 passed, 18 checks ([definition-campaign.json](definition-campaign.json)) |
| `PYC -m unittest discover app/runtime`; `python3 -m unittest discover -s scripts -p 'test_*.py'` | 160 OK; 17 OK |
| in `app/`: `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests --bins` | exit 0 (rust-version 1.85 floor holds) |
| `python3 Tickets/_tools/board.py check` | OK |

## Acceptance tests (ticket) and evidence

| Acceptance | Evidence (sharing campaign check) |
| --- | --- |
| The owner shares an app; the teammate opens it and sees data only through the teammate's own grants | 2 (exact subset, one event), 3 (the teammate lists and opens read-only; a command is Denied), 4 (`record_write` shared on request, then the command commits), 6 (a `share_work` holder shares only what it holds) |
| A non-member and an unauthenticated caller receive the uniform 404, identical for existing and nonexistent apps | 5 (self, revoked, outsider, unknown, another scope, no escalation: Denied, nothing written), 8 (byte-identical 404 bodies over HTTP) |
| Revoking the share removes access at the next request | 11 (operator `revoke-permission record_read`: the stream closes, the next request is Denied) |
| A shared app contains only admitted definitions; sharing grants no access the definitions do not carry | the share writes permission rows only, on the admitted S05 app, work and its recorded source dependency; the definer cannot name a permission or a resource the work does not carry (2, 6) |
| Real time (ADR-011 amendment) | 9 (delivery only to a subscriber who can open the work; nothing across organizations), 10 (the command route's hook and the composer hook), 11 (sign-out and revocation close the stream; one per session; the per-member bound) |

## Decisions in this slice (for review)

1. **Definer, not an application INSERT grant.** ADR-007 D7.1 makes `genui_admit` the authority writer; an INSERT grant on the permission tables to `genui_workspace` would have widened the grant set the schema campaign asserts literally. The definer keeps the application roles read-only on authority and computes the subset itself (D7.1: no permission names from callers, no bypass).
2. **`permission.granted` reused on the admission chain**, not a new kind: the operator `verify` already recomputes every chain generically and the operator channel's kind means the same thing; D7.8's closed list is amended by one RESTRICTIVE policy and recorded as debt.
3. **Owner-level = unrevoked creator or `share_work`.** No Phase 1 operator subcommand grants `share_work` (`GRANTABLE` excludes it); the campaign writes that row by fixture SQL as the operator stand-in.
4. **Epoch change closes the stream.** ADR-004 advances the epoch on every membership change (an operator revocation included), so a stream authorized at the earlier epoch ends at the next event and the client reconnects under a fresh session resolution; a sign-out ends it the same way.
5. **The hub is in-process** (ADR-007 D7.18 single task); the hook is a process-global `hub()` so the command route needed one line and no constructor change.

## Limits

- Local password authentication over TLS, not RDS IAM; synthetic identities; sessions minted by the `cfg(test)` constructor after the seeded membership (the session campaign owns the fAuth path).
- No browser: the stream is read as a body stream through `tower::ServiceExt::oneshot`; the canvas (`pivot/canvas`) subscribes to `/v1/events` with `EventSource`.
- The members list shows neutral labels (no display name exists in 0002); the sharer picks a principal id.
- An open stream does not refresh the session's idle clock and a session's absolute expiry does not close it before the one-hour lifetime.
- No image was built; the image contexts were edited (allowlists) but not exported.
- No independent review; nothing merged.
