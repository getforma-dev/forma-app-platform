# APP-002 contract qualification evidence

Date: 2026-10-28. Implementation edition `impl-2026-10-28.1`; source edition `prep-2026-10-22.1`. Source and artifact identities are in [implementation-lock.json](../../contracts/implementation-lock.json). The exact commands and output of a clean isolated dependency installation are retained in [local-validation.json](local-validation.json).

The qualification validates the same structural contracts for representative Northstar and Spend Review fixtures, their identity/reference relationships, explicit malformed cases, and canonical bytes/hashes in Python and independently in JavaScript. It does not run either application or demonstrate actual publication, authorization, database concurrency, parser execution or business arithmetic. The fixture names and descriptive placeholder evidence hashes are synthetic; they are not acceptance evidence for the later two-app journey.

Checks completed locally:

- Fresh virtual environment installs every pinned wheel using `--require-hashes --only-binary=:all:`; `pip check` reports no broken requirements.
- The contract suite passes 17 test methods, including 57 named negative vectors and 6 canonical byte/hash vectors. The methods additionally cover two distinct app bundles, all registered renderer tags structurally, owner-publication versus independent-review shape, job completion representation, literal text handling, depth/size bounds and payload identity.
- JavaScript independently reproduces all six canonical byte/hash vectors.
- The contract lock verifies all listed artifacts and both original source identities. The source schema matches the emitted JSON Schema artifact.

The first test run exposed a real validation gap: an impossible date (`2026-02-30T12:01:00Z`) reached `datetime.fromisoformat` and raised `ValueError: day is out of range for month` rather than the contract refusal. `jsonschema`'s base install does not ensure its optional date-time checker exists. The fix explicitly registers standard-library calendar checkers; the negative vector now fails at schema validation. A separate invalid month, offset timestamp and fractional timestamp are also refused under this edition's exact UTC-second grammar. The failed test and correction are retained here rather than reported as a first-run pass.

One registry-metadata request through the system Python failed local CA verification. Retrying with the OS `curl` trust store succeeded; TLS verification was never disabled. [dependencies.json](dependencies.json) records exact PyPI release sources and license metadata; wheel hashes are retained in the requirements lock.

The initial evidence was captured on macOS/Python 3.11.0/Node 22.21.0. The added CI uses Python 3.11.14 and Node 22.21.0 on Ubuntu 24.04 with SHA-pinned setup actions. Its outcome must be recorded separately after the change is pushed; local evidence does not claim that CI has passed.

Deferred proof obligations remain explicit: runtime admissions/registries, secure random key generation, current access checks, fenced commits and same-key retry behavior, source/parser integrity, query correctness/deadlines, five-second display revocation, managed RDS/S3/KMS integration and all phase release trials. See [ADR-002](../../docs/architecture/ADR-002-CONTRACT-EDITION.md) and [contract scope](../../contracts/README.md).
