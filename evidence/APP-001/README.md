# APP-001 qualification record — 2026-10-28

Acceptance mapping:

1. `registry.json`: eight downloaded archives match registry SHA-512/SHA-256; exact version/tag identity. Actual selected manifests and npm/Cargo lockfiles are under `spikes/APP-001/minimal`. Optional router is available at 3.2.0 but unused.
2. `docs/architecture/ADR-001-RELEASED-BASELINE.md`: dated disposition of 3.0 assumptions, budgets, state caps, rendering/security boundaries and each upstream gate.
3. `checks.json` + `checks.txt`: exact clean-install/type/build/Rust test commands and exit status. `http.json`: live loopback raw SSR, no-JS page, CSP/nonce/no-store evidence and emitted sizes. No browser adoption or full application acceptance is claimed.

Reproduce from root with `python3 spikes/APP-001/verify_registry.py` and `python3 spikes/APP-001/verify.py`. Read the harness README for prerequisites. No paid services or credentials are used. All servers start on loopback port zero and are stopped by their owning probe.

## Retained failed attempts and dispositions

- The generator rejects a slash-containing project name. Reran from `spikes/APP-001` with the valid name `minimal`; no framework change required.
- The generated starter lockfile, updated incrementally after exact pins, failed npm 11.6.2 `ci` with EUSAGE and missing Babel/alien-signals transitive entries. The subsequent typecheck/build failed because installation had failed. Preserved the intermediate in `spikes/APP-001/package-lock.incomplete.json`; regenerated the active lock from package.json with `npm install --package-lock-only --ignore-scripts`, then clean `npm ci`, typecheck and build passed. This is a scoped consumer resolution; no claim that the generator was fixed.
- Python's local urllib TLS trust store could not validate the registry certificate. The archive verifier uses system curl with normal certificate verification; no insecure flag or trust bypass.
- First `cargo fmt --check` caught wrapping in the newly added startup-guard test. Applied rustfmt and reran the locked test/fmt checks successfully.

Independent review and landing identity are added after the concrete diff is assessed. The phase gate remains pending.
