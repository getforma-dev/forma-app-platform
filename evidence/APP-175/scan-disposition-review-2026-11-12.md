# APP-175 scan disposition: independent review (2026-11-12 UTC)

**Reviewer:** an independent Claude subagent reviewer. It did not write this change. Review date 2026-11-12 UTC.
**Reviewed:** commit `cf9600e` ("APP-175: scan disposition for the schema release") on `app175/schema-release`, parent `62d3009`. It is local only: PR #90's head is still `62d3009`, so no CI has run on `cf9600e`.
**Guide:** `Tickets/_tools/REVIEW.md`. The guard is `guard_scan_delta` in `ops/promotion/registry.py`, called from `runner.py` at prepare, at execute and in the final checks. `ops/promotion/scan.py` does not exist.

## Checks run

1. **Recomputation.** A scratch script imported the guard's own `registry.finding_set`. It compared the private fresh scans (`scan-{service,qualification,operator}.json`) with the private `state.json` `live.registry` findings, and read nothing else from that file. Results for each role:
   - 37 current findings against 23 baseline: 14 introduced, 0 removed, 0 CRITICAL.
   - The introduced set equals the committed JSON's findings exactly. The raw `{name, severity, attributes}` entries are byte-equal after a sort.
   - The predecessor digest equals both the scanned `imageId.imageDigest` and the state's `manifest_digest`.
   - Every finding's name, severity and package match `REVIEWED_DISPOSITION`.
   - The committed delta's introduced set, digests, counts and baseline scan times all match.
   - The real guard on the real scans returned 14/14 for every role under the `to` contract. It refused the `from` contract, and it refused a clock set exactly at the expiry.
2. **Pins.**
   - JSON contract = `REVIEWED_SCHEMA_RELEASE['to']` = `schema-release.json` `to`, and that differs from `from`.
   - The expiry is `2026-11-20T00:00:00Z` in all four places.
   - The evidence path matches.
   - The file's SHA-256 is `e6016996…562213`, as the record states.
3. **Tests.** `python3 -m unittest discover -s ops/promotion -p 'test_*.py'` ran 177 tests: OK (at `62d3009`: 174, OK). All 13 scan tests and both schema/scan lifecycle tests ran with `-v`; none was skipped.
4. **Mutations**, made on a scratch copy of `cf9600e`, never on the worktree.

   Killed: dropping the contract or predecessor binding (`test_scope_and_expiry_refuse`); expiry `<` changed to `<=`; dropping the subset (widening) check (22 failures); allowing duplicate keys; changing the JSON's operator predecessor, one service fingerprint, or the contract to `from` (`ReleaseBindingTests`, end to end); moving the registry and JSON expiry together to 11-21; dropping one CVE or changing one severity consistently in both.

   Survived, as at `62d3009`: dropping the guard's CRITICAL refusal, alone or with `publish()`'s; dropping the "ambiguous reviewed finding" check (equivalent: the file already forces unique names).
5. **Leaks.** No file this commit touches contains the real registry ID, which I checked by exact match without printing it. The added lines contain no 12-digit number, `dkr.ecr`, `amazonaws.com`, `arn:aws`, `s3://`, bucket name, `/Users/`, `/private/` or scratchpad path. The repository names and digests in those files were already public.
6. **Evidence spot checks.**
   - `app/operator/operator.py:233-235` connects with `sslmode="verify-full", gssencmode="disable"`.
   - Dockerfiles and lockfiles are identical between the live source `05db000` and `cd17f93`, apart from `app/operator/Dockerfile.dockerignore`.
   - The private ELF and tracker outputs agree with the regcomp and krb5 importer lists.
   - The APP-173 change is a 4-line append and deletes nothing.
   - `python3 Tickets/_tools/board.py check` passes.
7. **Test diffs.** Expectations now derive from the committed file, and no assertion was removed. The CVSS4 mutation became `*_SCORE`, and a `*_VECTOR` mutation was added. Both expiry mutations are still exact boundaries. `build()` keeps its default path: `pinned = spec()` and `INFRA`. Three tests are new. No coverage was weakened.

## Findings

**Blocker:** none.

**Major 1: the CVE-2026-119079 applicability text overstates the controls.** It is in `SCAN-DISPOSITION.md` under "Applicability", krb5.
- The text says: "The service uses tokio-postgres with IAM tokens." The service image (`app/Dockerfile`) also installs psycopg-binary. Its entrypoint, `runtime.py`, has `probe()` at `app/runtime/runtime.py:738`, reached through the `probe` and `qualify` modes. That function opens libpq with IAM tokens and **without** `gssencmode="disable"`, so libpq uses its default `prefer`, which looks for a Kerberos credential cache through `libgssapi_krb5`.
- The text also implies that `gssencmode="disable"` disables GSSAPI. It disables only GSS *encryption* negotiation. libpq still answers a server's GSS authentication request, because `require_auth` is not set.
- The conclusion probably holds: there is no Kerberos config or credential cache, RDS uses IAM auth, and the process runs as a non-root user on a read-only filesystem. But the record should state the path and these limits.

**Minor 1: the tracker sentence overclaims.** The sentence "lists every one as vulnerable or unfixed in bookworm and bookworm-security" does not hold as written. The tracker's bookworm row for krb5 is `?`, and the gcc-12 row for CVE-2026-115456 was not parsed, as the next sentence admits. Its table cell still says "vulnerable". Reword to "no fixed version found (apt-cache: no newer candidate)".

**Minor 2: the static-analysis outputs are not retained.**
- The "no wide-scanf importers" claim cannot be re-checked from the repository.
- The record does not list the symbols that were searched. It should say whether the `__isoc99_`/`__isoc23_` aliases of `swscanf`/`vfwscanf` were included.
- Retain the sanitized importer lists, or the command and symbol set.

**Minor 3 (pre-existing): CRITICAL is enforced in code but not by a test.** Removing both CRITICAL refusals passes all 177 tests, here and at `62d3009`. Add a test with a CRITICAL finding that is not introduced, present in both the baseline and the candidate.

## Limits

- I did not run `just check` or the campaigns, and I called no AWS or GitHub write.
- I did not rebuild or re-analyse the images; the applicability checks rest on the implementer's private outputs and on code inspection.
- Whether `probe`/`qualify` run in beta was not established.
- Whether the candidate images report the same fingerprints is checked only by the guard at runtime.

## Verdict

**Changes requested**, limited to the evidence record: fix Major 1 and Minor 1. The guard, the bindings, the committed data and the tests are correct. The committed disposition equals the recomputed introduced sets, and the refusals for widening, duplicates, shape, wrong contract, wrong predecessor and expiry are all still enforced.

## Recheck (2026-11-12 UTC)

**Scope:** the staged diff on top of `cf9600e`: `SCAN-DISPOSITION.md` and the new `test_scan.py` test. I ran checks on a scratch copy of `cf9600e` with the staged patch applied.
- **Suite:** 178 tests, OK. Removing the guard's CRITICAL `require` fails the new test once per role (3 failures, "Refused not raised"). Removing it together with `publish()`'s check does the same. Minor 3 is resolved.
- **Major 1: resolved.** The record now names the `probe()` path and separates GSS encryption from GSS authentication.
  - The libpq claims match libpq's design. It starts GSS authentication only when the server sends AuthenticationGSS; non-Windows builds treat AuthenticationSSPI the same way, but a Linux RDS server does not send it. Under `prefer`, it attempts GSS encryption only after `pg_GSS_have_cred_cache` finds credentials.
  - I checked these from libpq's design, not by re-reading its source here.
  - `infra/src/stack.ts` sets no `KRB5*` variable. Its only writable mounts are `/run/genui` and `/run/operator`, so the default `/tmp/krb5cc_65532` cannot exist.
- **Minor 1: resolved.** The tracker text now records the `?` row and the unparsed gcc-12 row.
- **Minor 2: resolved.** The record now has the method, the six symbol names with both prefix families, and the per-image file counts. I did not reproduce the search.
- **New minor:** the sentence "Every connection here authenticates with an IAM token as a password" is wrong for `genui_migration`. The operator and `runtime.py` migrate use the Secrets Manager password `PGPASSWORD` (`infra/src/stack.ts:459,617`). That is still password authentication over verify-full, so the conclusion stands. Suggested wording: "with a password (an IAM token, or the migration secret)".
- **Leak check:** the staged additions contain no account ID, host, ARN or private path.

**Final verdict: no blocking findings.** The one new minor is a wording fix that can land with this change.
