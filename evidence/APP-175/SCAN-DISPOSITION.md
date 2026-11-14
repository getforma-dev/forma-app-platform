# November 12 scan disposition (APP-175 schema release)

## Renewal, 2026-11-14: 16 findings (supersedes the counts and hash below)

Release A's first preparation, run [43555596848](https://github.com/getforma-dev/forma-app-platform/actions/runs/43555596848) for `e1e710e`, refused at 04:57:32 UTC after publishing the three images. The scans completed at 04:55:27–04:57:23 UTC. No template or change set was created. Each candidate reports 38 findings (16 HIGH, 16 MEDIUM, 5 LOW, 1 UNDEFINED) against the canonical baseline's 23. Against that baseline it introduces the 14 findings below **and two more**, identical in every role and none CRITICAL. One baseline fingerprint is removed. The guard refused because the two extra findings are outside the November 12 set.

The disposition is regenerated with `ops/promotion/scan_renewal.py` (W3's tool, carried byte-identically from `trial/w3-ops` `c7ecd5ad`). Its inputs are, privately, the canonical state's `live.registry.<role>` and each candidate image's `describe-image-scan-findings`. The output is the [sanitized delta](scan-delta-2026-11-14.json) (`sha256 8897313b…`). Nothing was retyped. The scope is otherwise unchanged:
- the same contract `14a161a0…`, since the deployment contract hashes neither `ops/promotion` nor `evidence`;
- the same three predecessors;
- the same expiry, 2026-11-20 00:00 UTC, which is still the schema release's own.

| Finding | Severity | Package | What changed | Applicability |
| --- | --- | --- | --- | --- |
| [CVE-2026-76844](https://security-tracker.debian.org/tracker/CVE-2026-76844) | HIGH, CVSS 4 8.3 | zlib 1.2.13.dfsg-1 | **Not new.** It was accepted by the [APP-173 disposition](../APP-173/SCAN-DISPOSITION.md) and has been a baseline finding since the November 5 release. The scanner re-vectored it: the same name, package, version and score, with the CVSS 4 vector changed from `AC:H/UI:N/VC:N/VI:H/VA:H` to `AC:L/UI:P/VC:H/VI:N/VA:N/SC:H`. The fingerprint includes attributes, so the old one counts as removed and the new one as introduced. | APP-173's analysis is unchanged, because the package is unchanged. The only importers of zlib's gzip file API are `dpkg-deb` and `libapt-pkg`, which run at build time only. The scanner's new description names webpack-dev-middleware, which matches neither the package it attributes nor the CVSS change. webpack-dev-middleware is in no image: it is absent from `app/admin/package-lock.json`, and the Node stage of `app/Dockerfile` is a build stage whose only output is `admin/dist`, compiled into the service binary. |
| [CVE-2026-91094](https://security-tracker.debian.org/tracker/CVE-2026-91094) | UNDEFINED (no score) | dpkg 1.21.23 | Newly listed against the pinned `dpkg=1.21.23`. | The flaw is in unpacking `3.0 (quilt)` **source** packages, which is the job of `dpkg-source` (`Dpkg::Source`, shipped in `dpkg-dev`/`libdpkg-perl`). Neither is installed: a local build of the qualification image has `dpkg` 1.21.23 and `perl-base`, with no `/usr/bin/dpkg-source` and no `/usr/share/perl5/Dpkg`. The three final stages share the same `python:3.11.14-slim-bookworm` base and the same `--only-upgrade` package list, which installs nothing new. dpkg runs only during the image build. |

**Debian status** (security tracker, read 2026-11-14 at 05:26 UTC):
- **CVE-2026-76844 (zlib):**
  - vulnerable in bookworm `1:1.2.13.dfsg-1`, and there is no bookworm-security row;
  - fixed in trixie only through trixie-security (`1:1.3.dfsg+really1.3.1-1+deb13u1`, DSA-6613-1) and in forky/sid (`1:1.3.dfsg+really1.3.2-4`);
  - the tracker notes the same fixing change as CVE-2026-85091.
- **CVE-2026-91094 (dpkg):**
  - vulnerable in bookworm `1.21.23`, trixie `1.22.22` and forky `1.23.11`; fixed only in sid `1.23.12`;
  - trixie is marked `<no-dsa> (Minor issue)`;
  - there is no DSA or DLA.

No bookworm fix exists for either. Waiting for Debian is therefore not an option before this disposition's expiry, and pinning newer packages would change the Dockerfiles and so the deployment contract.

All containers run non-root (`65532:65532`) with a read-only root filesystem.

**Limits.**
- The dpkg check used an earlier local amd64 build of the qualification image (`forma248-local:qualification`), not the ECR ARM64 images. Its `/etc/debian_version` reads 12.15.
- The webpack-dev-middleware check is a lockfile search.
- Static analysis, as before, does not prove against `dlopen`.

**Checks.**
- `python3 -m ops.promotion.scan_renewal check --delta evidence/APP-175/scan-delta-2026-11-14.json` prints `"status": "bound"`, 16 findings, `disposition_sha256 f00adb9a…`.
- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 183 tests, OK.
- The tests that bound the count to 14 now bind 16. `test_scan.py` binds the delta's one removed fingerprint and requires CVE-2026-76844 among the introduced.

**Approval.** For each image the beta approval summary must show:
- 16 introduced, 16 reviewed, 0 unreviewed;
- this disposition's content hash, `f00adb9a047ce880535966ef5551d543b611be31c144be08efbc3e0345b5025b`;
- the expiry, 2026-11-20.

Approve only if those match. If any other finding appears, preparation stops again before approval is requested.

The November 12 text below is kept as written; its 14 findings are a subset of these 16.


**Scope.** This disposition covers only the APP-175 schema release (20 → 22). It is bound to:

- that release's `to` deployment contract `14a161a09cf93044cbf6d1281b02b56f954fa7f73ae3a17a8f54b07b7958d99b`, which the scan guard now checks for the candidate being promoted;
- the three live images as predecessors:
  - service `sha256:1bbf0e4db54db2bc898a5fd19f4042bfbd6e17331382a6d269b4052e532ac466`
  - qualification `sha256:152088e1d77bd0c15621a896951ff70ac9b6cb2e6f6033b0a8742d6d9c8233a0`
  - operator `sha256:9dbe284eb1f244f4b85c2beb741e7e18ba31607023aee98a497182ddbf59518a`
- an expiry of **2026-11-20 00:00 UTC**, the schema release's own.

It lets each image introduce the same 14 findings: six HIGH, six MEDIUM and two LOW, none CRITICAL. The exact fingerprints are in [`scan-disposition.json`](../../ops/promotion/scan-disposition.json). The source pins in `ops/promotion/registry.py` are this record, the three roles, the expiry and each CVE's severity and package, so editing the JSON alone cannot widen them. Any other finding, severity, package, predecessor, contract or later date refuses, and a CRITICAL finding is never accepted. Once the release succeeds, these become ordinary baseline findings; the disposition grants nothing to any later finding.

This replaces the [APP-173 disposition](../APP-173/SCAN-DISPOSITION.md) (zlib and apt, expired 2026-11-12), whose findings became baseline findings with the November 5 release. It is proposed on `app175/schema-release` (PR #90) and has effect only once reviewed and merged. It is temporary acceptance, not a package fix, a scanner suppression or a claim of zero vulnerabilities.

## Method

On 2026-11-12 at about 03:50 UTC, with Victor's permission, ECR `start-image-scan` ran on each live image manifest. Each scan completed at 03:49:59–03:50:00 UTC, with vulnerability data updated at the same time. Each image now reports 37 findings (16 HIGH, 16 MEDIUM, 5 LOW) against the 23 (10 HIGH, 10 MEDIUM, 3 LOW) recorded when the November 5 release scanned it.

The introduced set is computed with the promotion guard's own fingerprint, `registry.finding_set` (name, severity, sorted attributes), against the canonical baseline findings: 14 in every image, identical across the three, none removed. The disposition file and the [sanitized delta](scan-delta-2026-11-12.json) were generated programmatically from those scan objects. They copy only name, severity and attributes into the disposition; the delta also keeps each finding's NVD URI and description, plus the repository, digests and counts. Nothing was retyped. The registry ID and every other private value stay out. The images are unchanged since November 5; this is new scanner data about the same packages.

## Accepted findings

| Finding | Severity (CVSS 3) | Package | Component | Debian |
| --- | --- | --- | --- | --- |
| [CVE-2026-112314](https://security-tracker.debian.org/tracker/CVE-2026-112314) | HIGH 7.5 | krb5 1.20.1-2+deb12u5 | `lib/rpc/clnt_tcp.c` (GSS-RPC) | vulnerable |
| [CVE-2026-112315](https://security-tracker.debian.org/tracker/CVE-2026-112315) | MEDIUM 5.3 | krb5 | `lib/rpc/authgss_prot.c` | vulnerable |
| [CVE-2026-112316](https://security-tracker.debian.org/tracker/CVE-2026-112316) | MEDIUM 6.5 | krb5 | `lib/kadm5/kadm_rpc_xdr.c` | vulnerable |
| [CVE-2026-112317](https://security-tracker.debian.org/tracker/CVE-2026-112317) | MEDIUM 5.3 | krb5 | `lib/kdb/keytab.c` | vulnerable |
| [CVE-2026-112318](https://security-tracker.debian.org/tracker/CVE-2026-112318) | MEDIUM 6.5 | krb5 | `kadmin/server/server_stubs.c` | vulnerable |
| [CVE-2026-112319](https://security-tracker.debian.org/tracker/CVE-2026-112319) | MEDIUM 6.5 | krb5 | GSS-RPC XDR | vulnerable |
| [CVE-2026-112320](https://security-tracker.debian.org/tracker/CVE-2026-112320) | HIGH 7.5 | krb5 | GSS-RPC transport | vulnerable |
| [CVE-2026-112321](https://security-tracker.debian.org/tracker/CVE-2026-112321) | HIGH 7.5 | krb5 | `lib/rpc/svc_auth_gssapi.c` | vulnerable |
| [CVE-2026-112322](https://security-tracker.debian.org/tracker/CVE-2026-112322) | HIGH 7.5 | krb5 | `lib/rpc/svc_auth_gssapi.c` | vulnerable |
| [CVE-2026-112323](https://security-tracker.debian.org/tracker/CVE-2026-112323) | HIGH 7.5 | krb5 | `lib/rpc/svc_auth_gssapi.c` | vulnerable |
| [CVE-2026-119079](https://security-tracker.debian.org/tracker/CVE-2026-119079) | HIGH 7.5 | krb5 | `lib/krb5/asn.1` decoder (in `libkrb5`) | vulnerable |
| [CVE-2026-100290](https://security-tracker.debian.org/tracker/CVE-2026-100290) | LOW 3.7 | glibc 2.36-9+deb12u14 | `regcomp`/`regexec` | vulnerable |
| [CVE-2026-115819](https://security-tracker.debian.org/tracker/CVE-2026-115819) | MEDIUM 4.8 | glibc | wide `scanf` family | vulnerable |
| [CVE-2026-115456](https://security-tracker.debian.org/tracker/CVE-2026-115456) | LOW 3.7 | gcc-12 12.2.0-14+deb12u1 | libstdc++ | vulnerable |

The Debian security tracker, read on 2026-11-12, shows no fixed version for this base. The automated read parsed krb5 in bookworm-security (`1.20.1-2+deb12u5`) as vulnerable; its bookworm row read `?`, so that suite's status was not established. It parsed glibc as vulnerable in both suites. For CVE-2026-115456 it parsed only the gcc-11 source row (vulnerable); the gcc-12 row was not parsed. On ARM64, `apt-cache` shows no newer candidate for `libkrb5-3`, `libgssapi-krb5-2`, `libk5crypto3`, `libkrb5support0`, `libc6`, `libc-bin`, `gcc-12-base`, `libgcc-s1` or `libstdc++6`.

## Applicability

These checks used local images built from the same Dockerfiles at `cd17f93`.

- **krb5, 10 of 11.** Only the four krb5 client libraries are installed: `libkrb5-3`, `libgssapi-krb5-2`, `libk5crypto3` and `libkrb5support0`. `libgssrpc`, `libkadm5`, `libkdb5`, `kadmind` and `krb5kdc` are absent, so the `lib/rpc`, `kadmin` and `kdb` code these ten CVEs name is not in the images.
- **krb5, CVE-2026-119079.** Its decoder is in the installed `libkrb5`. The same importers appear in all three images:
  - Debian's `libgssapi_krb5` and the SPAKE preauthentication plugin import `libkrb5` directly;
  - `libtirpc` reaches it through `libgssapi_krb5`;
  - psycopg's bundled `libgssapi_krb5` and `libsasl2` import its bundled `libkrb5`, which its `libpq` reaches through `libgssapi_krb5`.

  Each is reached only through GSSAPI or Kerberos, and nothing here authenticates with Kerberos.
  - The service's database client is tokio-postgres with IAM tokens; it does not link libpq.
  - Two psycopg/libpq paths exist. The operator connects with `gssencmode="disable"` (APP-084). The service image's `runtime.py` `probe()` does not set `gssencmode`, so libpq's default (`prefer`) applies.
  - `gssencmode="disable"` turns off GSS *encryption* negotiation only, not GSS *authentication*. libpq starts GSS authentication only when the server asks for it (`AuthenticationGSS`, which needs a `gss` `pg_hba` entry). Every connection here authenticates with a password over `sslmode=verify-full`: the application roles with IAM tokens, and `genui_migration` with its Secrets Manager password (`infra/src/stack.ts`).
  - Under `prefer`, libpq attempts GSS encryption only when it finds Kerberos credentials (`pg_GSS_have_cred_cache`). The containers hold no credential cache or keytab, and the root filesystem is read-only.
  - Limits: this rests on libpq's documented behavior and the RDS configuration (no Kerberos directory integration). The RDS `pg_hba` was not read.
- **glibc, CVE-2026-100290.** `regcomp`/`regexec` are imported by system utilities: `grep`, `bash`, `diff`, `more`, `run-parts`, `hardlink`, `lsfd` and `localedef`. The other importers are `libapt-pkg`, `libapt-private`, `libsemanage`, `libformw`, and both `libkrb5` copies (Debian's and psycopg's). The entrypoints are `python3` and the Rust binary. Python's `re` and Rust's `regex` do not use glibc's regex.
- **glibc, CVE-2026-115819.** No file outside glibc itself names a wide `scanf` function. Method: each image's filesystem was exported and every executable or shared-object file (1,035–1,038 per image) was searched for `wscanf`, `fwscanf`, `swscanf`, `vwscanf`, `vfwscanf` and `vswscanf`, with or without the `__isoc99_`/`__isoc23_` prefixes. `libc.so.6`, which defines them, was the only match. This is a string search of the dynamic symbol names, not a proof of reachability.
- **gcc-12, CVE-2026-115456.** A vendor-open libstdc++ finding of the same class as the [APP-167 disposition](../APP-167/SCAN-DISPOSITION.md)'s gcc-12 findings.
- **All containers** are non-root (`65532:65532`) with a read-only root filesystem (`infra/src`).

**Limits.** The images analysed were local builds, not the ECR images, though they come from the same Dockerfiles and pinned base. This is static import analysis, not proof against `dlopen`. The krb5 version inside psycopg's bundled copies was not identified; the scanner attributes these CVEs to the Debian package.

## Checks

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'` passes.
- `test_scan.py` now derives the reviewed names, counts and expiry from the committed file. It keeps the tamper suite, which refuses CRITICAL, widened, duplicated and reshaped files and the superseded APP-173 evidence path. It binds the disposition to the schema release: the same `to` contract and expiry. It also binds it to this record's delta: the same predecessors and the same fingerprints for every role.
- `test_schema_release.py` promotes the committed schema release with the committed disposition end to end in fake AWS. Each image reports 14 introduced, 14 reviewed and 0 unreviewed, and execution admits the new contract.
- **Not bindable in code:** that each predecessor is the live image, because the canonical state is private, and that the candidate images report the same findings. The guard checks both at preparation and again before the outage.

## Approval

For each image the beta approval summary will show:

- **14 introduced, 14 reviewed, 0 unreviewed**, with the 14 CVE IDs;
- this disposition's content hash, `e60169964ed009b973dd24a342b10fe3a211e3dc297bb0ab411fc3b496562213` for the file as committed;
- the expiry, 2026-11-20.

Approve only if those match. Any other introduced finding still stops preparation before approval is requested.

## Independent review

[scan-disposition-review-2026-11-12.md](scan-disposition-review-2026-11-12.md) reviewed `cf9600e`: changes requested to the evidence text only. Dispositions by the lead, 2026-11-12:

- Major (CVE-2026-119079, the service's psycopg probe and what `gssencmode` disables): fixed above. The record now names the `runtime.py` `probe()` path, distinguishes GSS encryption from GSS authentication, and states its limits.
- Minor (tracker overclaim): fixed. The krb5 bookworm row is recorded as `?`, and the gcc-12 row as not parsed.
- Minor (wide-`scanf` method): fixed. The method, the six symbol names, the per-image file counts and the single `libc.so.6` match are recorded.
- Minor (recheck: `genui_migration` uses its Secrets Manager password, not an IAM token): fixed in the sentence above.
- Minor (no test isolates the CRITICAL refusal): fixed. `test_a_critical_finding_refuses_even_when_the_predecessor_had_it` puts the same CRITICAL finding in both the baseline and the current scan. Deleting the guard's CRITICAL `require` fails it once per role (3 failures); the full suite is 178 tests OK.
