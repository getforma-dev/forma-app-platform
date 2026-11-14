# Review of the 2026-11-14 scan disposition renewal (PR #118)

**Reviewer:** an independent Fable subagent, read-only, about 05:25 UTC on 2026-11-14. It decided in place of Victor, under his delegation of 2026-11-14 ("please don't ask me ask fable and have it review decisions and code and the plan").

**Head reviewed:** `94a66b2a`.

**Verdict:** GO WITH CHANGES. There is no blocker and no major finding. Every requested change is to prose, so the disposition (`f00adb9a…`), the delta (`8897313b…`) and the registry pin stand unchanged.

## What the reviewer verified itself

1. **The delta is faithful to its inputs.**
   - `scan_renewal delta` was rerun on the private inputs (each role's canonical `live.registry` entry and its candidate's `describe-image-scan-findings`). The output is byte-identical to `scan-delta-2026-11-14.json`.
   - `generate` from that delta is byte-identical to `ops/promotion/scan-disposition.json`.
   - The printed pin is byte-identical to `REVIEWED_DISPOSITION` in `registry.py`.
   - None of the 58 distinct private identifiers in the inputs (account IDs, ARNs, registry hosts) occurs in any changed file.
2. **The scope did not widen.**
   - `deployment_contract_digest` at the PR head and at `e1e710e` both equal `14a161a0…`, which is `schema-release.json`'s `to`.
   - The predecessors equal the live manifest digests.
   - The expiry is still `2026-11-20T00:00:00Z`.
   - No current scan has a CRITICAL finding.
   - The tamper suite still refuses all 29 mutations.
3. **The inputs match the record.**
   - Each role has 38 findings (16 HIGH, 16 MEDIUM, 5 LOW, 1 UNDEFINED) against the baseline's 23.
   - 16 findings are introduced, with the same names in every role.
   - Each role has exactly one removed fingerprint: CVE-2026-76844 with its November 5 vector. The package, version and score 8.3 are unchanged.
   - The scans completed at 04:55:27–04:57:23 UTC, and the refusal was logged at 04:57:32 UTC.
4. **The checks pass.** The promotion suite (183 tests) is OK. `scan_renewal check` reports `bound`. `scan_renewal.py` is byte-identical to `c7ecd5ad`. `test_scan_renewal.py` differs from it in two lines: the delta path and the count, 14 → 16.
5. **The applicability claims hold.**
   - All three final stages use the same pinned `python:3.11.14-slim-bookworm` base with the same `--only-upgrade` block, and run as `USER 65532:65532`.
   - There is no `webpack-dev-middleware` in `app/admin/package-lock.json`.
   - The local `forma248-local:qualification` image has `dpkg 1.21.23` installed. It has no `dpkg-dev`, no `libdpkg-perl`, no `dpkg-source` and no `Dpkg` Perl modules. It has `zlib1g 1:1.2.13.dfsg-1`.
6. **The abandon preconditions are met.**
   - The phase is `preparing` with no packet.
   - All 181 journal entries are confirmed and inside the registry-only allowlist, with no change-set, stop, activate or schema entries.
   - The window ID equals the promotion ID and is in the ledger.
   - The reconcile basis is valid until 04:13:33 UTC on Nov 15.

## On the decision

Accepting both findings is justified for a beta serving 25 real adults, and no check of the ECR images themselves is required before merge.

- **CVE-2026-76844 is not a new vulnerability.** It has the same package, version and score as the reviewed November 5 acceptance, and has been live since then. Only the scanner's vector changed.
- **CVE-2026-91094 is in `dpkg-source`,** which is absent from the images. It has no CVSS score, and Debian rates it no-dsa / minor.
- **Blocking release A is not a real option.** Neither CVE has a bookworm fix. Changing the package pins would change the deployment contract, which needs its own admission, all before the Nov 20 expiry.

## Requested changes and their resolution

| # | Severity | Requested | Resolution |
| --- | --- | --- | --- |
| 1 | MINOR | Add Debian's tracker status for both CVEs to the record. | Done in `29a34641`, from a fresh read of the tracker at 05:26 UTC. |
| 2 | MINOR | The PR body overclaims that both tool files are byte-identical. | PR body corrected. W3's later merge will conflict on `test_scan_renewal.py` and must keep this branch's binding to the 11-14 delta and 16 findings. |
| 3 | MINOR | State the fence anchor: the run's `updated_at` 04:57:35Z gives a fence of 06:02:35Z (`maintenance.terminal_proof`). Add `inspect` before abandon, and an `inspect` readback of `active: null` before re-enabling. | PR body corrected; the operator follows it. |
| 4 | MINOR | `test_scan.py` binds the removed count, but only a comment names the removed fingerprint. The delta format records no removed fingerprints. | No change. The reviewer verified it against the private inputs, and the delta format is W3's tool. |

## Rollout timing

- Approve outside 05:00–05:45 UTC.
- Execute must start before 04:13:33 UTC on Nov 15, and in any case before Nov 20 00:00 UTC.
- Preparation after the first merge took about 44 minutes.
- Fresh candidates are rescanned, so any further change in the scanner's data refuses again before approval is requested.
