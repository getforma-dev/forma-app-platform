# November 5 scan disposition

Reviewed scope: the next beta promotion from the live release `beta/20261104-1e28488`. It covers exactly two HIGH findings, in each of the three images, bound to each image's live predecessor manifest, the deployment contract `4a2cea85…` and an expiry of **2026-11-12 00:00 UTC**. The exact fingerprints are in [`scan-disposition.json`](../../ops/promotion/scan-disposition.json); the accepted names, severities and roles are also pinned in `ops/promotion/registry.py`, so editing the JSON alone cannot widen them.

This is a temporary acceptance, not a package fix, a scanner suppression or a claim of zero vulnerabilities. A changed CVSS value, package version, severity, predecessor image or deployment contract falls outside it. A CRITICAL finding is never accepted. After a successful release records its scan inventory, these two become ordinary baseline findings; this disposition grants no permission for any later new finding.

Selected by Victor, 2026-11-05, after he was shown the three options (dispose now, fix apt first, wait for Debian).

## What happened

| Time (UTC) | Event |
| --- | --- |
| 10:49:06 | Run [42013842380](https://github.com/getforma-dev/forma-app-platform/actions/runs/42013842380) starts for merged `f745ba7` (PR #85). |
| 11:28–11:30 | Preparation passes the contract check, takes the deploy lock, publishes all three images and reads their scans. Each image has 26 findings against its baseline's 24: two introduced, none removed, zero CRITICAL. The guard refuses (`new scan findings require separately reviewed disposition`). No template or change set was created. |
| 11:30:41 | The run completes as failed. The lock stays held (`phase: preparing`, zero unresolved operations, state `08b8e5ed…`). Beta keeps serving `beta/20261104-1e28488`; `/`, `/health/live` and `/health/ready` returned 200. |
| 16:47:49 | Victor runs the documented `maintenance abandon --expected-state-sha256 08b8e5ed…` after the 65-minute fence (12:35:41). Result: `abandoned`, promotion `a87b389b-43d0-4687-aace-8ce342706a55`, 32 windows preserved. `inspect` then reads `active: null`, revision 5949, state `16d9dafb…`. |
| 16:57:22 | `PROMOTION_ENABLED` set to `false`, as for the November 2 PR40–42 batch, so merges create no doomed runs. |
| 16:57:57, 17:01:04 | PR #86 merged as `32e6ac9`, then PR #87 as `b16fcc5` (after the combined tree passed `cargo clippy --all-targets -D warnings` and the board check); their promotion runs [42069893935](https://github.com/getforma-dev/forma-app-platform/actions/runs/42069893935) and [42070412728](https://github.com/getforma-dev/forma-app-platform/actions/runs/42070412728) were skipped. |

The live images were scanned on November 4 between 22:15 and 22:20 UTC, before the scanner's data listed these CVEs. They contain the same `apt` 2.6.1 and `zlib1g` 1:1.2.13.dfsg-1, so the finding is new data about unchanged packages, not a regression in our build. Per-role digests, counts and the raw introduced findings: [scan-delta-2026-11-05.json](scan-delta-2026-11-05.json).

## Accepted findings

| Finding | Package | Debian status (security tracker, 2026-11-05) | Disposition |
| --- | --- | --- | --- |
| [CVE-2026-76844](https://security-tracker.debian.org/tracker/CVE-2026-76844) HIGH, CVSS 4 8.3 | `zlib1g` 1:1.2.13.dfsg-1 | Vulnerable in bookworm, trixie and forky; fixed only in sid (`1:1.3.dfsg+really1.3.2-4`). Debian notes the same fixing change as CVE-2026-85091. | **No vendor fix exists for Bookworm.** Accepted with the applicability evidence below. CVE-2026-85091 (same root cause) is already a retained baseline finding, assessed in [APP-140](../APP-140/IMAGE-SECURITY.md). |
| [CVE-2026-109518](https://security-tracker.debian.org/tracker/CVE-2026-109518) HIGH, CVSS 4 7.7 | `apt` 2.6.1 (`libapt-pkg6.0` 2.6.1) | Fixed in bookworm-security `2.6.2` (DLA-4880-1); [ARM64 candidate confirmed](arm64-apt-policy.txt). | **A fix exists**, but pinning it changes all three Dockerfiles and therefore the deployment contract, which automatic promotion refuses and `main` has no admission procedure for. Accepted temporarily; the fix is tracked in [APP-089](../../Tickets/0-backlog/APP-089.md). |

## Applicability

**zlib, CVE-2026-76844.** The overflow is in `gz_write`, reached through zlib's gzip *file* API (`gzwrite`, `gzfwrite`, `gzputs`, `gzprintf`) after a failed `write()`.

- In all three images (820, 819 and 820 ELF files), the only importers of any `gz*` function are `/usr/bin/dpkg-deb` and `libapt-pkg.so.6.0.0`; `libz.so.1.2.13` is listed because it defines them. The service binary, `parser-host`, Python, DuckDB and psycopg import none. [Output](elf-gz-imports.txt) and [script](elf-gz-imports.sh).
- Python's `gzip` and `zlib` modules use zlib's stream API (`deflate`/`inflate`), not the file API.
- No application source calls a `gz*` function or loads zlib through `ctypes` (`git grep` over `app`, `ops`, `parser`, `contracts`, excluding vendored code).
- `dpkg-deb` and APT run only during the image build. Every ECS container definition in `infra/src/stack.ts` and `infra/src/shared-broker.ts` sets `readonlyRootFilesystem: true` and `user: '65532:65532'`, and the live service and broker task definitions read back the same values on 2026-11-05.

Limit: the images were built locally at `cd17f93` for this check, not pulled from ECR; they come from the same Dockerfiles and pinned base. Static import analysis does not prove that no library resolves `gzwrite` dynamically through `dlsym`; no such use was found.

**apt, CVE-2026-109518.** The attack needs an APT client to install packages over HTTP through an attacker-controlled path, with `Acquire::Retries` non-zero.

- At runtime it does not apply: no source invokes `apt`, `apt-get` or `dpkg`, the containers run as UID 65532 with a read-only root filesystem, and the service exposes no shell or package operation.
- **At build time it does apply.** Each Dockerfile runs `apt-get install --only-upgrade` with the base image's APT 2.6.1 and Bookworm's default `Acquire::Retries=3`, over HTTP from `deb.debian.org`. An attacker on the path between the CI runner and Debian's mirror could, in principle, substitute a package. Pinning `apt=2.6.2` alone would not close this, because 2.6.1 performs that install. The fix belongs in APP-089: build with `-o Acquire::Retries=0` (or upgrade APT first in its own step), pin 2.6.2, and admit the changed deployment contract through a reviewed procedure.

## Checks

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 124 tests, OK. New tests show that changing any finding's name, severity, CVSS score, package name or version refuses; an extra finding refuses; a changed predecessor, contract or expiry refuses for each role; one role alone may introduce the reviewed pair; and sixteen edits to the JSON file (CRITICAL or MEDIUM severity, a new CVE, an extra or missing finding, a missing or extra role, a malformed digest or date, schema 1, the APP-167 evidence path) all refuse.
- `deployment_contract_digest(repo, 'HEAD')` on this branch is `4a2cea855e962fda7f79a55f2b2754df9a056e4b3bab3fc4e5b4ee1a98fdd2a0`, the live baseline's value.
- The live state (read-only) names the same three predecessor manifests as the disposition, and its November budget basis was reconciled at 2026-11-04 21:28:12 UTC, so preparation needs a fresh basis after 2026-11-05 21:28 UTC.

## Approval

The beta approval summary will show, for each image, **2 introduced, 2 reviewed, 0 unreviewed**, the two CVE IDs, this disposition's content hash and its expiry. Approve only if those match. Any other introduced finding still stops preparation before approval is requested.
