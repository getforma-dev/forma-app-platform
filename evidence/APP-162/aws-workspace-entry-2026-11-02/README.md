# Workspace-entry AWS release

Candidate `7c3ab123f524c4e9a7b855be0a2f582c626341ae` is live at **https://beta.getforma.dev**, activated **2026-11-02 03:29:22 UTC** (November 1 in New York). [PR 39](https://github.com/getforma-dev/forma-app-platform/pull/39) merged it as `0fe030c7624bc29b31a7e659771f323281847420`; their source trees are identical. Pushed annotated tag `beta/20261102-7c3ab12` resolves to the exact qualified candidate.

## Delivered behavior

[Sign in](https://beta.getforma.dev/signin) accepts a workspace name, opaque handle or supported local sign-in link. Enter **forma-trial**, or use its [direct sign-in link](https://beta.getforma.dev/signin/forma-trial). Submitting the form starts the existing tenant-bound hosted authentication directly. Remembered and direct-link selections are editable, and **Switch workspace** opens an empty input. No anonymous directory, email discovery or universal account is implied.

The alias maps to the original trial's provisioned handle and organization. The existing trial was retained. No tenant recreation, database migration, snapshot, operator task, account/password reset, provider/secret change, seed, grant change or invitation replay occurred in this rollout. Restoring Northstar as another served workspace is separate work.

## Qualification and release evidence

- [Local qualification](../workspace-entry/README.md): 21 real PostgreSQL session checks, 13 invitation checks, eight shared-identity checks and four focused parser/security/native-rendering tests. The provider in these campaigns is synthetic.
- [Full repository check](full-check.json): all 20 steps passed, including 375 Rust library tests, eight native binary tests, three doctests, 103 frontend tests and all 34 operations tests.
- [Exact-candidate CI](ci.json): all 28 checks passed; all 24 push/PR workflows completed successfully on their first attempt, without reruns or cancellations.
- Independent authentication review passed. Independent operations review caught noncanonical base32 alias handles; a canonical round-trip check and regression repaired it. Duplicate JSON keys also refuse. A disposable release-test fixture was updated to include the new helper; the complete operations suite then passed.
- All three ARM64 images passed qualification. Every exported OCI blob was independently rehashed; actual registry index, unique ARM64 child and configuration digests match. Canonical image pushes succeeded without a transport fallback.
- [Runtime evidence](runtime.json): schema 19, application TD 16 and broker TD 4 healthy at `1/1/0`, all five verified-TLS probes and readiness passed. The actual change set contained 105 resources and 10 Modify entries, with no Add/Remove. Its only changes were three image parameters, the expiry, and the exact alias environment on application/rollback task definitions. Both services were proven stopped before exact-ARN execution and again before activation.

The final independent runtime audit passed and rehashed all 56 aggregate receipts, including actual running image identity, exact-candidate CI, merge/tag binding and ledger continuity. Its private receipt hash is recorded in `runtime.json`.

All 15 prior cost records and allowances were retained. The new canonical forecast is $169.84 GenUI / $207.84 combined under the existing $170/$208 caps; both enabled stop schedules target November 30 at 14:00 UTC. Runtime and CloudFormation receipts, credentials, account identifiers and browser profiles remain private; this packet contains selected outcomes and evidence hashes.

## Scanner disposition

Service and qualification scans each report 24 findings: zero Critical, eight High, 13 Medium and three Low. Compared with the previous scans, three Medium GCC findings were added and one High sqlite entry disappeared. Operator's unchanged ARM64 child retains 22 findings. All 105 installed package/version/architecture tuples are identical between old and new images in each role; this is not a dependency upgrade or a vulnerability fix. The new reports are retained, with no new High/Critical finding. Debian lists the affected GCC version as vulnerable and no fixed package for [CVE-2026-114690](https://security-tracker.debian.org/tracker/CVE-2026-114690), [CVE-2026-114691](https://security-tracker.debian.org/tracker/CVE-2026-114691) and [CVE-2026-115455](https://security-tracker.debian.org/tracker/CVE-2026-115455). No claim of unreachable vulnerable code or vulnerability-free images is made.

## Product acceptance boundary

Local native Chrome checks passed desktop and 320 px layout, keyboard navigation, required-field validation, useful neutral retry, editable direct links and switching. [Live browser entry verification](hosted-entry.json) passed in an isolated in-app browser: submitting `forma-trial` reached the correct existing tenant login; returning to sign-in retained an editable alias; Switch workspace displayed an empty field; the direct link displayed the editable alias. This is separate from infrastructure health. This release does not repeat a credential login after the user's reported password reset and does not reuse the old stored password.

APP-163 owns verified-email workspace discovery; APP-164 owns authenticated multi-workspace switching; APP-165 owns real workspace-aware invitation emails. Current provider management invitations are intentionally server-only, so successful verification/reset email delivery is not evidence of invitation-email delivery. Genuine recipient signup/acceptance and the timed recording remain separate obligations.
