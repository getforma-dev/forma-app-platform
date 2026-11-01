# Composed-app operator packaging check

The local image build for `e67968088028a857fbe9920a352afa758ddbc342` was stopped before qualification after review found that the operator Dockerfile omitted `contracts/record_components_contract.py`. Its build context already allowed the file, and `validate.py` imports it when validating the new record-components receipt. Allowlist coverage alone did not prove the module was packaged. No registry push or AWS mutation occurred.

The Dockerfile now copies the helper. The operator image qualification runs its own packaged validator with a member receipt and a guest receipt, then checks that a schema-valid, re-digested receipt with invalid budget provenance is refused. These are synthetic contract vectors, not evidence of hosted record activity or a new operator command.

The focused regression stages only the contract files named by the actual Dockerfile COPY instructions. Both valid receipts pass and the semantic violation refuses. Removing the helper reproduces `ModuleNotFoundError` using the same probe. The 18 operator package tests passed in 3.06 seconds:

```sh
app/operator/.venv/bin/python -m unittest discover -s app/operator -p test_operator.py -k PackageTests -v
```

[The receipt](operator-packaging.json) retains source and test-log hashes. Independent source review found no authority or command-surface expansion. Actual execution of the new probe inside the next clean candidate's ARM64 image is still required, together with all three image qualifiers, exact-head CI and the reviewed deployment steps. The stopped candidate is unqualified; the live catalog-18 deployment remains unchanged.

## Foundation CI timeout

The same candidate's [Foundation push run](https://github.com/getforma-dev/forma-app-platform/actions/runs/41362433962) failed when the runtime unittest command exceeded the scaffold runner's 300-second limit. The wrapper discarded its buffered output on timeout, so the retained CI log cannot identify the last test or establish an assertion failure. The full suite had taken approximately 227 seconds locally.

Only this runtime suite now has a bounded 600-second allowance; all other scaffold commands retain 300 seconds and the tests are unchanged. Timeout and ordinary failure output is written into `local-preparation.json` and printed before dependent steps stop. Three focused regressions cover byte-valued timeout output, nonzero failures and successful progression. The infra test wrapper and TypeScript check also pass. [The timeout receipt](foundation-timeout.json) preserves the original failure and correction provenance. A successful new exact-head CI run is still required.
