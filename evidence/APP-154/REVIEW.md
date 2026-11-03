
# APP-154 independent review

## Root review: identity, grants, migration and user journey

The root reviewer did not author the shared identity/session implementation, account/tenant migration or grant/placement storage implementation. Review covered the separate OAuth cookies and CSRF binding, verified issuer/tenant/subject/mailbox proof, exact-share callback routing, home-session preservation, epoch changes, bounded session and flow cleanup, account RLS, sponsor delegation, idempotency, audit, query attribution, placement lifetime and final release checks. No member `TrustedRequest` or organization grant is manufactured for a guest.

Findings fixed and independently rechecked:

- Email changes originally preserved the account epoch. A changed verified mailbox now increments it, invalidating old cookies and in-flight account authority while retaining immutable provider identity. The native campaign covers this.
- Canvas permission removal originally left a reference capable of reappearing after reenable. Placement now pins the grant version and a placement generation; a fresh explicit Add is required, and stale inventory cleanup cannot delete a newer placement. The native campaign covers toggling without an intermediate poll, cleanup/re-add ordering and expired-reference quota reclamation.
- Recent revoked history could hide an older active share at the list limit. Active/unexpired grants now sort before history, preserving owner management of the bounded live set.
- The first browser create failed because storage accepted command-style 32-hex idempotency keys while the sharing UI/API used UUIDs. Sharing now consistently uses UUIDs; default placement=false and expiry=7 days are server defaults. The browser lost-response test verifies one recovered grant with the same key and payload.
- The bounded cleanup definer has no caller-controlled selector, a fixed search path, at most 100 dead flows and 100 dead sessions per call, and returns only vault references. It uses the existing NOLOGIN definer role, which application roles cannot assume. Account scope is forced RLS; the data role cannot mutate these relations. Historical migrations and logical catalogs 1–16 remain unchanged.
- Browser review found mobile min-content overflow and unreadable connected-card text inherited from the canvas theme. Scoped CSS corrections were followed by another browser run and visual inspection. The final evidence records no mobile overflow or page errors.

Native PostgreSQL/HTTP sharing campaign: **13 grouped checks passed**. Schema, grants, catalog and lock qualification: **381 checks passed**. The source inventory and historical contract-edition assertions were corrected after the full repository checks exposed stale expected values; final check results are recorded in the delivery README rather than treating the initial red run as success.

The root also authored HTTP guest document/release and receipt integration and the initial operator broker. Those surfaces are independently reviewed below. The broker's final deployment byte/version bounds are separately rechecked after the correction; no live AWS provisioning is part of this review.

## Independent review by review_page: shared HTTP, guest reads, broker and freshness

Scope: `app/src/sharing/live.rs`, `app/src/storage/shared_reads.rs`, operator invitation broker, and the frontend authority/connected-app controllers. This reviewer authored the separate shared identity/session implementation, so the identity code requires review by the root reviewer; its test results below are author verification.

Findings and dispositions:

- Guest document release occurs after query execution and again after the connected-app lookup, before JSON or native HTML emission. Denied guest reads abort the response, rather than preserving previously read values in a partial successful document. No release-order defect remained in the reviewed HTTP source.
- Guest read plans carry the actual account, share and grant version. They never construct an owner `TrustedRequest`. Work/publication/definition pins and admitted view/collection membership are rechecked; command metadata is removed and record links are absent from shared props.
- Found missing per-account write limits and ignored mutation bodies on accept/link/unlink. The root reviewer added an account write limiter and a bounded, closed empty JSON body; unknown fields and unsupported content types now refuse.
- Found missing publication policy-version comparison at guest query release, relative to the member release contract. Fixed by re-reading the admitted publication premise at query release and capturing policy_epoch in request-scoped GuestAccess, then comparing it plus operator_version at final document release. Independent source re-review found no remaining blocker; the storage campaign now also exercises a policy-version change between query authorization and release.
- Broker source keeps the existing management key in an isolated operator process, pins the actual provider tenant, accepts only the fixed `user`/`server_only` operation, persists pending state before the non-idempotent provider POST, and does not blindly repeat unknown outcomes. Found permanent capacity accumulation of expired confirmed entries and ambiguous duplicate authentication/content-type headers; root added confirmed-expiry reclamation and duplicate-header refusal. Unknown outcomes remain retained for reconciliation.
- Reviewed the cleanup definer: no caller-selected scope or arguments, at most 100 flows and 100 sessions per invocation, only dead session vault references returned, `SKIP LOCKED`, pinned search path, and an existing NOLOGIN definer role which application roles cannot assume. It does not return account data or mutate accounts, shares or tenant grants.
- Found live controls that could stay stale after another tab accepted an invitation or canvas permission became available. The UI reviewer fixed them. Authority freshness is independently checked with a lease anchored at request start; data fetch completion cannot extend it.

Independent executed checks:

- `app/operator/.venv/bin/python -I -m unittest discover -s app/operator -p test_shared_invite_broker.py -v`: **10 passed**, including HTTP fake-provider retry/restart, durable unknown outcome, concurrent idempotency, expiry reclamation and duplicate headers. Local contract fakes only; no AWS call or hosted-provider signup delivery was exercised.
- `cd app/admin && npx tsx --test test/authority.test.ts test/connected-authority.test.ts`: **5 passed**. Fake-clock coverage includes delayed/superseded/repeated/invalidated verdicts, malformed authority, offline/visibility masking, old JSON bodies after reconnect, expiry during a stalled request and disposal. Connected inventory failure hides values without removing/remounting the item; the successful inventory retry preserves its original geometry.
- `cargo check --locked -p forma-app-platform --features dev-stand`: **passed** for the dev identity integration at that point. The final combined check is reported separately.

Author verification for shared authentication:

- `cargo test --locked -p forma-app-platform --lib auth::shared -- --nocapture`: **4 passed, 1 ignored** (the ignored test is the separately invoked native campaign). Verifier-backed local HTTP tests cover wrong mailbox/tenant/issuer/subject, email-verification gating, closed form intents, exact-share CSRF, uniform failure and preservation of home cookies.
- `GENUI_NATIVE_FIXTURE_PYTHON=$PWD/app/runtime/.venv/bin/python GENUI_SHARED_IDENTITY_REPORT=/tmp/forma-shared-identity-campaign.json cargo test --locked -p forma-app-platform --lib auth::shared_tests::native_shared_identity_campaign -- --ignored --exact --nocapture`: **passed, 6 grouped checks, 12.97 seconds**. Retained result: [shared-identity-campaign.json](shared-identity-campaign.json). It exercises production routes/storage against real local PostgreSQL, with local HTTP provider and broker contract fakes. Checks cover exact-work cross-org identity reauthentication with home-session preservation, verified signup returning the exact invitation without membership, unverified refusal/pending/fresh sign-in, wrong identity/issuer/tenant/subject and callback replay, full-cap session replacement, shared-only signout, ordinary signout clearing both identities, and revoked-share refusal before provider authorization. Tenant principal/membership/entitlement/grant/app/work counts remain unchanged by external account onboarding.

Limits: all provider/broker tests use explicitly synthetic local HTTP services. The dev-only guest listener injects a synthetic account identity and tests product sharing routes; it does not test hosted signup. Production still requires private broker deployment/configuration, the existing operator management credential, a dedicated state secret, and registration of the dedicated shared OAuth callback. No deployment or AWS verification is claimed here.

Final disposition for this reviewer: no unresolved source blocker remains in the assigned HTTP/guest-read/broker/freshness scope. The missing policy-version release fence is fixed and the native storage campaign now passes its policy-change regression. Root independently reviewed the identity implementation; production provider/broker deployment remains an explicit unverified operational gate.

## Final broker bounds correction and root re-review

Root review identified that the initial state bound/version write rate did not match Secrets Manager's operational limits. The identity reviewer implemented a 60 KiB UTF-8 state bound, 256-byte completion headroom and a durable 40-write rolling 24-hour reservation budget. A new provider create reserves both pending and completion writes before any provider request; alias creation reserves one, exact replay none. Budget exhaustion survives restart, retains future reservations across clock rollback and refuses before provider contact. The root reviewed the state serializer, reservation order and SDK-stub/fake-provider tests; all **16 broker tests passed** in the final targeted checks. The operational runbook includes this deliberately small MVP bound and the new closed initialization shape. No AWS call or hosted delivery is implied.
