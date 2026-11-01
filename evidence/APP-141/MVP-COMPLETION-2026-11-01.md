# APP-141/142 — actual templates and independent saved apps

The MVP follow-up began on trunk `9ba2061` and merged through [PR 31](https://github.com/getforma-dev/forma-app-platform/pull/31), reviewed head `711d87d`, merge `6c308f1`. All 28 exact-head checks and the full local check passed. Source delivery and separate hosted acceptance are tracked in [MVP gap completion](../../docs/planning/MVP-GAPS-2026-11-01.md).

## Implemented

Four server-owned, admitted templates start with empty collections. Catalog discovery is authenticated and read-only. Confirmation sends a closed template identifier and description. Unchanged text instantiates the definition without a model; customized text includes that definition in model context and must pass normal admission/publication/save. Both retain Adapted provenance. Successful creation opens one united app page, following admitted part order and widths.

A display name belongs to the exact saved work. Additive migration 0018 supplies nullable display_name and optimistic name_version, with column-only workspace UPDATE authority. The fenced owner/admin write locks one row, rechecks entitlement after waiting, and audits the change. Publication, work version, records and share pins are unchanged. Authorized document, canvas, shared and command/listing reads use that name; seeded Library aliases keep their established app-wide scope.

## Validation and independent review

- `just campaign composer`: 15 checks passed. The native PostgreSQL campaign creates an unseeded member's first shell only on explicit compose, creates two distinct empty template works without a model, and verifies declared guide-first layout and persisted Adapted provenance.
- The same campaign verifies sibling name isolation, reload/discovery/command titles, serial/batched listing parity, foreign and ungranted denials, stale/concurrent version conflicts and an unchanged work authority version. Expiry while blocked on the work row denies the write with no name/version or audit change. See `native-template-names.json`.
- Isolated browser evidence in [APP-143](../APP-143/README.md) covers the actual creation modal, automatic Unite, declared guide/form order, form node/draft/idempotency-key preservation through fan-out, and name Save/no-op behavior.
- Independent reviewers found and corrected a missing entitlement recheck after the row wait, stale command/listing titles, and a client which misreported an unchanged Save. Tests retain those regressions.
- Integration also corrected the catalog's mount outside the long-running compose route, private audit hashing, selected enum options and image context inclusion. No authorization or timing limit was weakened.

The local identities and data are synthetic. Real provider signup and the clean hosted first-user journey remain APP-139/140 acceptance, not a consequence of these checks.

## Rename propagation through an existing live share

[Retained browser proof](rename-share-2026-11-01/README.md) starts with an already-active named guest share and connected placement. The real Library UI rename reached the already-open recipient title after 2,105 ms and connected title after 1,096 ms, without navigation. Share ID/status/version/expiry/add permission, work publication/work version, sibling title/publication/work version and connected geometry stayed unchanged. The source write changes only name metadata; this focused browser test compares those identities rather than taking a new exhaustive record-byte snapshot. The four before/after screenshots were visually reviewed and contain synthetic fixture data, without visible email addresses or credentials. This closes local rename-through-share acceptance, not genuine-provider onboarding.

The later [genuine hosted owner journey](../APP-139/hosted-owner-2026-11-01/README.md) closes the previously missing real-provider first-member observation: empty authoring-only admission, verification/fresh sign-in, customized template creation with zero records, then the first audited record. It complements native read-only/cancel/refusal checks. New-recipient and recording gates remain open.
