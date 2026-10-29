# Canonical consumer live refresh campaign

Prepared local executable only. APP-006 and FORMA-248 hosted acceptance remain open. No hosted request, bearer token, cloud window or deployment is supplied by this directory. This narrowly answers FORMA-248's unknown-key refresh requirement: normal rotation prepublishes the next key, so acceptance of that cached next key cannot prove a new unknown-key fetch.

The operator-only [native example](../../../spikes/APP-006/examples/hosted_key_qualification.rs) uses the exact canonical `genui_auth_verifier::Verifier`, production HTTPS constructor, normal system clock and one instance for the entire process. The library's new coherent diagnostics expose bounded counters only; reading them cannot fetch, change freshness or reset the refresh budget. A reserved attempt may be canceled before reaching the issuer, so it is labeled an attempt. An accepted keyset additionally establishes a successfully parsed response. Neither counter alone proves an external proxy/access-log request count.

The five-step campaign is:

1. Verify one newly issued synthetic access token and exact expected provider tenant/subject. Require at least 100 seconds remaining and counters 1 attempt/1 accepted set.
2. Replace only its header kid with a random unknown value, retaining claims/signature. Require `RefreshThrottled` and unchanged counters.
3. Wait at least 61 actual monotonic seconds from successful warm verification, then verify the same control. Require `UnknownKey`, 2 attempts/2 accepted sets and generation 2. No clock/cache manipulation or process restart occurs.
4. Repeat that same unknown control immediately. Require `RefreshThrottled` and unchanged counters.
5. Verify the original valid token again with the same identity and unchanged counters. A failed step ends this campaign; there is no retry-to-pass behavior.

A random collision with a real second key fails this trial; it cannot produce a false unknown-key success. Both original and forged token remain private in process memory. They are never sent to the JWKS endpoint, written to output or hashed into evidence. This is canonical native consumer qualification against the actual hosted JWKS, **not** an HTTP callback, app session, membership, mailbox, KMS custody, live key rotation or fleet-retirement test. The shared provider campaign must retain its separate checks.

## Private input contract

Provide exactly one closed JSON document through a private anonymous stdin pipe and close the writer. Do not use shell arguments, environment variables, shell tracing, a terminal or a regular file. Input is capped at 16 KiB and must finish within 2 seconds; whole campaign execution is bounded to 90 seconds. The process creates no credential files, listeners, child processes or cloud resources. On timeout it exits, including when the writer withholds EOF or a canceled DNS lookup is still pending. A separate fixed process watchdog covers synchronous executable hashing and blocked output too. Exit 124 without a complete result is a failed run; exit 125 means the watchdog could not be armed. The operator must never interpret missing or partial JSON as success.

| Field | Required value |
| --- | --- |
| `schema_version`, `synthetic_tenant` | `1`, `true` |
| `run_id` | Nonempty ASCII letters/digits/hyphen/underscore, at most 64 characters |
| `declared_consumer_revision`, `declared_provider_revision` | Actual reviewed 40-character lowercase hex source pins; declarations are explicitly not process attestations |
| `expected_tenant`, `expected_subject` | Provisioned `ten_` / `usr_` plus canonical lowercase UUID; never substitute app organization UUIDs |
| `issuer`, `audience` | Literal `gatewasm`, `gatewasm-app` |
| `jwks_url` | Exactly `https://auth-staging.getforma.dev/auth/jwks` |
| `max_access_lifetime_seconds` | `900` |
| `expires_at` | Integer Unix time 90–180 seconds after launch, rechecked during/final campaign |
| `access_token` | Dedicated fresh synthetic bearer, at most 8192 bytes; separate from continuity/rotation sessions |

The provider's existing reviewed operator runner should create the envelope in memory and supply it directly to the child stdin pipe. This document grants no start permission. Actual provider IDs/token may be produced only after coordinated provisioning in the agreed interval. No public token/debug route, weakened issuer policy or new long-lived secret is needed.

The sanitized result contains fixed cases/refusals/counters, monotonic elapsed time, process ID, OS/architecture, compiled verifier/campaign source hashes, declared source pins and the on-disk executable digest. The last digest describes the disk file, not process attestation. A separate operator receipt must bind the actual image/architecture digest, deployment or local execution environment, reviewed source hashes, start/finish/cleanup and provider endpoint revision. Never claim the previously published callback image contains this newer executable.

## Local reproduction and image

From repository root:

```sh
CARGO_BUILD_JOBS=1 python3 spikes/APP-006/qualify.py --output evidence/APP-006/hosted-key-qualification/local-new
docker build --file spikes/APP-006/Dockerfile --tag genui-auth-key-qualification:local spikes/APP-006
```

The first command runs 26 canonical tests, two example tests and 13 actual private-pipe subprocess negatives, including the real 90-second blocked-output watchdog. These use no hosted issuer or real token. Normal workflow qualification includes these checks. The image is a separate nonroot one-shot executable with a six-file allowlisted context and pinned existing base-image digests. Execute with an attached stdin pipe, read-only root, dropped capabilities and no new privileges; actual hosted execution needs HTTPS egress. Local negative-image tests use no network. No service port or app route is exposed.

Review, exact source CI, Linux/image qualification, registry publication and the coordinated hosted result must be retained separately. Local checks or this plan do not pass the hosted row. The existing $5 auth reservation is already included in the project monthly reservation; do not add a second allocation or start a new compute window without the current cumulative ledger and operator coordination.

## Recorded local preparation

[Local run 2](local-2/qualification.json) passes formatting, strict Clippy, 26 canonical tests, two example tests and all 13 real private-pipe cases. [Rust 1.85 compatibility](msrv.json) separately passes the locked all-target compiler check. [Actual Linux ARM64 image checks](image-local-1.json) pass three refusals under nonroot, read-only, no-network execution; all containers were removed. [Context export](context.json) records the exact six image inputs. [Independent review](independent-review.md) records the repaired watchdog finding, independently repeated process/image checks and final reviewed source hashes. The later runner-summary prose edit changes no executable input; CI must still bind the committed source before handoff.

These results do not include registry publication or any hosted request. The local image identifier is `sha256:5ee5ad165bca1e37bc2efe13f2fb727912c008cbe721b18f452a98482404cedc`; it is distinct from the earlier callback image. The provider may prepare its private in-memory envelope adapter from this contract, but must wait for the final reviewed image/source handoff and agreed interval before execution.
