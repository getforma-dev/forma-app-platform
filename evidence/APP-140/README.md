# APP-140 clean hosted trial evidence

The clean beta activated on November 1 at 20:13:41 UTC from exact candidate `711d87d`: application TD 13, broker TD 1 and catalog 18. [Actual activation evidence](hosted-activation-2026-11-01/README.md) records the healthy deployment, operator results, release tag and preserved verifier findings. The [genuine owner journey](../APP-139/hosted-owner-2026-11-01/README.md) separately passed; real new-recipient acceptance and recording remain open. The later [authentication repair delivery](shared-auth-fix-1d88470/README.md) activated exact `1d88470` at 21:53:16 UTC with healthy application TD 14/broker TD 2, the same catalog-18 manifest and no new migration or provisioning. Original data/share retention and genuine guest registration to the email-verification step are recorded in [the continuation](../APP-139/hosted-guest-entry-2026-11-01/README.md). Verified guest acceptance and recording remain open. The preparation sections below retain their earlier observation scope.

- [hosted-preparation.json](hosted-preparation.json): sanitized live baseline, fresh provider tenant/client registration, the one SES verification request, local budget reconciliation and remaining execution boundaries.
- [local-validation.json](local-validation.json): focused suite results. The full candidate check, CI and image identities belong to the integration delivery record.
- [Image security follow-up](IMAGE-SECURITY.md): exact ARM64 scan findings, available Debian fixes, package resolution and the remaining vendor-open findings. Final qualification and actual registry scans are recorded in [the candidate receipt](mvp-candidate-711d87d/README.md).
- [Clean deployment sequence](../../docs/pitch/CLEAN-TRIAL-DEPLOY.md): actual inputs, operator steps, empty first-member admission and activation checks.

## Changes and checks

The clean stack profile disables demo routing and personas, preserves old data, adds a fresh object boundary, and deploys the invitation broker as a separate bounded task. Its role cannot connect to the database or object store; only its dedicated state secret is writable. App/broker configuration arrives through managed secrets and becomes private local files after raw JSON is removed. The normal membership capability store remains unavailable to this broker.

Provisioning now creates only organization infrastructure: one space, one owner-publish policy, provider binding, audit heads, sequence and sign-in handle. The real PostgreSQL suite proves it creates no members, apps, works, content or grants, and repeat provisioning changes nothing. `issue --authoring-only` is the explicit no-resource-grant admission mode, exclusive with `--grant`. Real reserve/pending/redemption and replay produce one normal entitlement with zero resource or organization permissions. The member's later first compose uses APP-145's separately qualified creation path.

The runtime accepts a bounded set of up to 32 uniquely identified tenant credentials, allowing a fresh third organization without bypassing per-tenant role, key or prefix checks. Legacy duplicate/empty/oversized tenant sets refuse. The clean request composer requires one fresh organization, its exact provider binding and two distinct OAuth clients. Management credentials never reach the app task.

Activation verifies both exact task boundaries and stop schedules. Migration refuses until app and broker counts are all zero; release recording requires a completed broker rollout as well as application health. Tests include incorrect roles, credential channels, schedules, resources, counts and rollout state. The public ALB route is authenticated by the narrow token and bounded by the broker; it is not represented as private network isolation.

Commands run from the worktree (reports were retained privately during execution):

```sh
app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_runtime.py
app/runtime/.venv/bin/python -m unittest discover -s app/operator -p test_shared_invite_broker.py
app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_provider_invitations.py
app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_admission_operator.py
app/runtime/.venv/bin/python -m unittest discover -s infra/test -p '*_test.py'
npm --prefix infra run typecheck
npm --prefix infra test
app/runtime/.venv/bin/python -m unittest discover -s ops/beta -p 'test_*.py'
app/operator/.venv/bin/python -m unittest discover -s app/operator -p test_operator.py
bash -n ops/beta/deploy.sh
git diff --check
```

Runtime 55, broker 18, provider custody 50, real PostgreSQL admission 34, infra Python 80, infra TypeScript 45 and ops 12 passed. Operator package ran 39 with six image-dependent cases skipped. The final ARM64 image run must supply those cases; local package tests do not establish image readiness.

## Cost and remote actions

The authorized local reconciliation preserved every existing reservation and October entry. Only November basis/timestamp changed. Actual AWS reads establish live resources and the billing-lag limitation; the calculation keeps conservative reservations instead of treating estimated zero as free operation. No new deployment window was reserved during preparation. Pricing was checked against official AWS pages on November 1: Virginia x86 CPU `$0.000011244/second`, memory `$0.000001235/GB-second`, public IPv4 `$0.005/hour`, secrets `$0.40/month`, KMS keys `$1/month`. The code conservatively rounds the broker rate to `$0.0173428/hour`; its added retained resources total `$3.60/month`. At the recorded projection, the unchanged caps have only `$0.20` headroom.

Remote mutations were explicitly scoped and separately reported: one SES email-identity verification request; one fresh invite-only provider tenant, its two exact callback clients and new managed tenant-key secret. The provider setup was immediately read back, with zero users and zero invitations. No app deployment, database migration/provision, membership invitation, guest signup or app secret overwrite occurred in this preparation. All mailboxes, keys, provider capabilities and admission codes remain outside Git.

Independent source review found and closed the queued rename entitlement-expiry gap in APP-141. The shared OAuth client review confirmed strict separate-client validation and consistent PAR/exchange use. A second agent independently reviewed the provision/no-grant admission and isolated broker transport/custody boundary and reported no concrete blocker. Root owns the final integrated regression run and deployment decision.
