# Implementation contract edition impl-2026-10-28.1

APP-002 freezes an initial wire/data contract and qualifies its consistency. These are synthetic specimens and a validation harness, not application admission, storage, authorization, rendering, parser execution or query execution. APP-007 and later tickets implement those boundaries in the browser/Rust application.

The controlling shared source edition is `prep-2026-10-22.1`. `implementation-lock.json` names its exact digest, the frozen demo lock, this implementation's artifacts and [ADR-002](../docs/architecture/ADR-002-CONTRACT-EDITION.md). Frozen design bytes remain unchanged. The initial edition establishes a compatibility baseline; there is no previous implemented edition to claim an upgrade/downgrade test against.

## Files and reproduction

- `schema.py` is the concise schema source; `schema.json` is the emitted JSON Schema 2020-12 artifact for all consumers. The test checks they agree.
- `validate.py` applies the schema, canonical encoding and basic cross-artifact reference/type/digest checks. `validate_bundle` qualifies one fixture's consistency; its supplied context is trusted test input, never a way to authorize a request.
- `fixtures/northstar.json` and `fixtures/spend-review.json` use the same definition, publication, work, source, receipt and command/outcome shapes with different purposes and views. They are representative contract fixtures, not the full S01–S07 apps. Original source bytes, normalized Parquet, admission execution and actual result rows are absent: their descriptive placeholder digests are clearly synthetic. Definition, input-contract, record-schema, query, receipt and command-payload digests are computed and verified from the content present.
- `fixtures/negative-vectors.json` records independently named malformed and inconsistent changes. `canonical-vectors.json` stores explicit byte/hash expectations; `verify_vectors.mjs` checks those using a separate JavaScript implementation.
- `requirements.lock` pins the validator and its transitive dependencies with PyPI wheel hashes. Python 3.11 is the verified interpreter; Node 22+ supports the independent vector verifier. No npm dependency is needed.

From the repository root:

```sh
python3 -m venv contracts/.venv
contracts/.venv/bin/python -m pip install --require-hashes --only-binary=:all: -r contracts/requirements.lock
contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py' -v
node contracts/verify_vectors.mjs
contracts/.venv/bin/python contracts/lock.py check
```

When deliberately revising this edition before it lands, regenerate `schema.json` with `python3 contracts/schema.py`, regenerate synthetic examples with `contracts/.venv/bin/python contracts/make_fixtures.py`, rerun checks and update the lock with `contracts/.venv/bin/python contracts/lock.py write`. After the edition is in use, a compatibility change requires a new edition and review; changing a lock is not approval to reinterpret stored data.

## Identity, grammar and evidence

Stored app envelopes have organization and stable app IDs; revision, publication, work, source and record IDs use lowercase UUID v4/v7 syntax. Schema validation checks syntax, not uniqueness in a database or random entropy. Local field/view/input/command IDs use lowercase ASCII names and remain stable across revisions. Inputs in a reusable definition contain typed schema contracts. Publications bind definition, input and record-schema digests plus registered version identities; concrete source revisions and record collections belong only to saved-work bindings. A receipt records the concrete sources, record sequence, parameters, principal and policy/membership/filter epochs, query and result identities. Truncated pages remain distinct from incomplete totals; incomplete calculation is refused, never a partial number certified as a total.

The definition grammar has nine named renderer kinds and a bounded operator vocabulary, not executable source. There is no SQL, HTML/CSS, URL, path, role or grant field. Literal user text can contain apparent instructions or markup and is still data; later trusted renderers must escape it. Schema tags are contracts for planned registered capabilities, not proof those renderers/operators exist. Semantic validation checks references, units on literal filters/commands, and some operation constraints; full admission, query planning, dimensional arithmetic, source capabilities and policy decisions belong to their owning runtime tickets.

Exact business integers are canonical decimal strings up to 38 digits, avoiding browser precision loss. Decimal values use a signed coefficient, explicit scale 0–18 and unit. The first synthetic money contract admits USD with exponent 2 only; any additional currency/exponent is a reviewed contract extension, never a guessed conversion. Cross-currency arithmetic is not admitted. JSON numeric counters, versions and coordinates are safe integers only. Timestamp values are valid calendar instants in fixed-width UTC whole-second `YYYY-MM-DDTHH:MM:SSZ`; offsets, fractions and leap seconds are outside this initial edition. Date/time formats are explicitly checked using Python's standard library, so the optional `jsonschema` date-time dependency cannot silently disable validation.

The initial source formula policy is `reject`: cached formulas are not silently trusted. Source metadata contains parser/mapping versions, units/time confirmation, counts and bounded sample coordinates. Actual archive-wide bounds and retained original values are the parser/import tickets' responsibility.

## forma-json-v1 canonical encoding

This is a restricted encoding named **forma-json-v1**, not RFC 8785/JCS. It accepts null, booleans, scalar Unicode strings, safe signed JSON integers, ordered arrays and objects with ASCII keys matching `[a-z][a-z0-9_]*`. Object keys sort by ASCII byte order. UTF-8 strings retain their Unicode scalar sequence without normalization; escaping follows JSON for quotes, backslashes and controls. There is no whitespace. Arrays preserve order. Floating-point tokens, exponents, NaN/infinity, duplicate keys, unsafe numeric integers and unpaired surrogates are refused. Canonical bytes, not input whitespace/key order, determine identity. The decoder caps input at 1 MiB before parsing, then applies a depth-32 cap; excessive decoder recursion is classified as a refusal. This qualification harness is not a streaming hostile-input parser and makes no preallocation memory or deadline guarantee.

SHA-256 receives the ASCII domain string, one NUL byte, then canonical bytes. Registered domains are `forma:app-definition:v1`, `forma:input-contract:v1`, `forma:record-schema:v1`, `forma:query:v1`, `forma:query-receipt:v1`, `forma:command-payload:v1`, `forma:source-binding:v1` and `forma:result:v1`. The definition digest covers its immutable `content`; a receipt digest covers the complete receipt except its own `digest`. A command payload digest covers the complete request except `idempotency_key`; its identity scope is organization/principal/work/command/key as enforced later by storage. Every other domain hashes its complete named value. Hashes are content identity, not signatures or authorization.

The synthetic retry horizon is 24 hours from committed time. The outcome shape only represents a committed result; `accepted` is a job acknowledgment, and `unknown-to-client` is a transport/observation state reconciled by key rather than a new command. Same-key replay, ordering before expected-version checks, epoch fencing, audit atomicity and current outcome visibility still require real database/concurrency tests. The 32-hex key shape does not prove a client used a secure random generator. Local copies, schema validation and digest equality do not establish those guarantees.

## Source and dependency decisions

Traceability: shared C01–C16, especially C02/C04–C10; platform §§4.2,5.3,6.2–6.5,8,9,11.1; final demo D2/§8. [ADR-002](../docs/architecture/ADR-002-CONTRACT-EDITION.md) resolves the total revocation deadline, deployed stores and completion semantics. RDS/S3/KMS and live fAuth integration remain separate gates.

The only validation dependency is [jsonschema 4.26.0](https://pypi.org/project/jsonschema/4.26.0/), supporting Draft 2020-12; [its validation documentation](https://python-jsonschema.readthedocs.io/en/stable/validate/) explains explicit format checking. Exact transitive versions, registry sources and license metadata are retained in [dependencies.json](../evidence/APP-002/dependencies.json). Schema `$ref`s are local; no fixture controls a schema reference or causes a remote schema fetch.
