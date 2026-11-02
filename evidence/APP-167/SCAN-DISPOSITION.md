# First promotion scan disposition

Reviewed scope: the first APP-167 operator-image promotion from the exact
predecessor ARM64 manifest and deployment contract recorded in
[`scan-disposition.json`](../../ops/promotion/scan-disposition.json). Expires
**2026-11-09 00:00 UTC**, checked during preparation and again before execution.
This is a temporary acceptance of the three specified medium findings, not a
package fix, a scanner suppression, or a claim of zero vulnerabilities.

## Observed evidence

[Run 41395551487](https://github.com/getforma-dev/forma-app-platform/actions/runs/41395551487)
selected merged commit `365c88344b9367990d55e668e90a5dcf217dfe77`, passed all twelve
required push workflows, built and qualified three ARM64 images, authenticated
through the restricted OIDC preparation role, published the exact images and
completed their ECR scans. Preparation refused the operator scan delta before
creating a change set, requesting approval or changing the runtime. Its private
journal contains 178 confirmed ECR operations and no unresolved operation.

Each candidate image has 24 findings: zero critical, eight high, thirteen medium
and three low. Service and qualification introduced no findings against their
respective baselines. Operator introduced the three below and lost one older
finding against its 22-finding baseline. That baseline scan completed at
2026-11-02 00:19:37 UTC; service and qualification scans completed at 03:10:45
and 03:10:50 UTC and already contain these exact three findings. This timing is
consistent with scanner-data changes, but does not prove the deployed operator's
exposure. The original baseline remains unchanged.

The actual qualified OCI layers' dpkg inventories confirm `gcc-12-base`,
`libgcc-s1` and **`libstdc++6`**, all ARM64 version `12.2.0-14+deb12u1`, in both
operator and service. The affected runtime library is installed; absence of a
compiler or headers would not prove that compiled template code is unreachable.

## Exact accepted findings and limits

| Finding | Vendor report and current disposition |
| --- | --- |
| [CVE-2026-114690](https://security-tracker.debian.org/tracker/CVE-2026-114690) | Medium; libstdc++ parallel-search buffer bounds, with denial-of-service impact. Debian lists the installed Bookworm GCC 12 source version as vulnerable and unfixed. |
| [CVE-2026-114691](https://security-tracker.debian.org/tracker/CVE-2026-114691) | Medium; large `vector<bool>` construction can overflow internal size calculations. Debian lists the installed version as vulnerable and unfixed. |
| [CVE-2026-115455](https://security-tracker.debian.org/tracker/CVE-2026-115455) | Medium; wide-character bitset extraction can overflow a stack buffer. Debian lists the installed version as vulnerable and unfixed. |

The application and broker expose bounded application operations, not arbitrary
C++ execution. Query and parser dependencies still contain native code, and this
review does **not** establish transitive unreachability. The installed pinned
distribution has no vendor fix for these findings at this review. The temporary
disposition permits this compatible application release while retaining those
limitations and the existing runtime constraints. Any changed package version,
severity, CVSS attributes, predecessor image or deployment contract falls outside
the disposition. A critical finding is never accepted by this mechanism.

The private packet retains complete unsuppressed scan results. The one human
deployment approval displays **3 introduced, 3 reviewed, 0 unreviewed**, the CVE
IDs, disposition content hash and expiry. Other new findings still stop
preparation. Once a successful release records its actual scan inventory, later
unchanged findings are ordinary baseline findings; this disposition grants no
continuing permission for another new finding.

The failed preparation must be reconciled through the independently reviewed,
confirmed-ECR-only abandonment path after its workflow credentials expire. Its
ledger window, receipts and failure evidence are retained. It cannot be resumed
with different source or by editing its baseline. A fresh reviewed merge supplies
the next candidate and its normal single protected-environment approval.
