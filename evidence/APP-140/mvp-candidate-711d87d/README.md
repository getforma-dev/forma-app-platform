# Merged MVP candidate qualification

Source candidate `711d87d985cd0484bd25d2f863f6428d370881ea` merged to `pivot/pitch-mvp` through [PR 31](https://github.com/getforma-dev/forma-app-platform/pull/31) at `6c308f1a9c7db08d019332bb14b9b235db650362`, November 1, 2026, 19:48:13 UTC. [merge.json](merge.json) records public GitHub metadata. This folder records source and image qualification; it does not claim hosted activation, real signup or recording acceptance.

## Exact source checks

[checks.json](checks.json) retains all 28 successful checks at the exact candidate: 26 GitHub Actions jobs across push/PR runs, plus the two Socket checks. Both tenant-foundation tails passed, including schema/catalog/RLS, native TLS and identity/sharing campaigns. The [PR qualification comment](https://github.com/getforma-dev/forma-app-platform/pull/31#issuecomment-6562992202) records the check result and the two narrow repairs prompted by earlier failed heads.

[full-check.json](full-check.json) records `just check` exit 0 and every one of its 20 successful steps, with the original log hash. This includes strict frontend build/tests, Rust tests, clippy, development clippy, MSRV, HTTP/security smoke, infrastructure/runtime/operator checks, catalog/pins, contracts and board integrity. The execution owner observed exit 0; the retained derivative avoids local paths and runtime logs.

[native-postgres.json](native-postgres.json) retains the 36 passing real-TLS PostgreSQL checks. The targeted `just campaign postgres` run completed in 49.26 seconds after compilation. Its source-fixture repair derives the expected database version from the manifest, probes the unsupported next version, and restores the exact current constraint in both repair paths. The separate synthetic-reset repair adds only the schema-18 defaults `display_name: null` and `name_version: 0` to its exact expected row comparison. Neither change weakens a production check.

## ARM64 images and actual registry comparison

[image-qualification.json](image-qualification.json) identifies service, qualification and operator images built from the exact candidate. All three local image qualifications passed. Parser identity matched between service and operator. [runtime-package-readback.json](runtime-package-readback.json) verifies the installed pins from each immutable image without network access or image modification.

The completed registry comparison was read at 19:38:50 UTC from the reviewed deployment window's scan receipts. [image-scan-comparison.json](image-scan-comparison.json) retains only candidate/digest/finding facts and the original comparison hash, excluding registry account paths and private operator state. For **each** image, the scan removed exactly the expected 25 findings (14 high, nine medium, two low), introduced zero, and retained **24 findings: 11 high, 10 medium and three low; zero critical**. The vendor-open findings remain in the unsuppressed inventory. Their applicability limits are documented in [the security review](../IMAGE-SECURITY.md); they are not declared fixed.

| Image | Immutable Linux ARM64 child |
| --- | --- |
| Service | `sha256:d6016fe41d82a73a605c71006ebc9e45535442a033349c20dd0512752d12b26d` |
| Qualification | `sha256:6eaf29aea94210eed0fd1c8349fa1584655c7fb41f164b8be51069aca70fb7da` |
| Operator | `sha256:a6b8eb6645d46e6ecccd2598719a9bdb98ca5b68c35fde3975d74991b189573a` |

The older triage document and failed-head evidence retain their original timestamps. This final comparison supersedes their pending rebuild/rescan state. The [later actual activation record](../hosted-activation-2026-11-01/README.md) independently proves schema migration and application/broker health. Real owner/guest identity and product progression remain separate; none follows automatically from merge, image qualification or deployment.
