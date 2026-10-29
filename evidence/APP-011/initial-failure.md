# Retained fixture-verifier failure

Command: `python3 -m unittest discover -s fixtures -v`

First run: eight tests, two failures. The original-file tamper and duplicate/path-escape tests received `symlink_fixture_path` before reaching their intended checks. The verifier inspected ancestors outside the chosen fixture root, so macOS's normal `/var` → `/private/var` alias prevented temporary-directory verification.

Correction: resolve the explicitly selected root once; reject symbolic links among its descendants. Keep containment/path checks and add a real in-package symlink negative test. This changes fixture verification only, not the untrusted-input parser or an application authorization boundary. The later qualification report records the rerun.

The first full qualification (`local-1/summary.json`) later failed because the fixture expected a formula refusal code of `formula_rejected`; the actual qualified parser correctly returned `formula_refused` and `no_revision_created: true`. All 26 accepted inputs had already matched their original values and coordinates. The formula source bytes did not change; the manifest expectation was corrected to the actual parser contract. Both refused-input raw reports remain in `local-1/parser/`. This was a harness expectation error, not a parser acceptance defect.
