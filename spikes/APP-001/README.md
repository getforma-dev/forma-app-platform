# APP-001 released baseline harness

Isolated derivative of `@getforma/create-app@3.2.0` minimal, created on 2026-10-28. This is a qualification fixture, not the application scaffold (APP-007), full canvas (APP-003) or an auth integration.

From repository root, with Node 22+, npm 11, Rust 1.93.1 and Python 3.10+:

```sh
python3 spikes/APP-001/verify_registry.py
python3 spikes/APP-001/verify.py
```

The first command verifies the eight published archives and release tag. Router is checked for a matching release but is not installed or adopted. The second uses the committed npm/Cargo lockfiles, builds frontend before Rust, tests required SSR startup modes and probes an owned loopback server on an OS-assigned port. It terminates that process afterward. Reports go to `evidence/APP-001/`; expanded build output is ignored. No browser adoption, managed storage or full-library performance is claimed.

Exact creation: in this directory, `npx --yes @getforma/create-app@3.2.0 minimal --template minimal`. Application-specific derivative changes: exact Forma pins; native flags in build/dev; required native route guard; focused refusal check; typecheck command. The initial starter lock did not survive npm 11's consistency check after exact pins were applied. `package-lock.incomplete.json` preserves that failed intermediate; the active lock was regenerated from the manifest then verified with clean `npm ci`. See evidence for details.

To discard this qualification, remove only this directory and its evidence after preserving review. No application data or framework source is affected. Never copy generated `dist` files into an app or relax strict mode to admit failed source.
