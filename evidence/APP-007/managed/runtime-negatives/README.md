# APP-007 managed negative runner preparation

This directory records **local preparation only**. No managed trial was performed by this author. Root owns the reviewed one-off task registration, actual run, immutable image binding, stopped-task receipt and temporary task-definition cleanup. The optional APP-006 callback/wiring image remains unchanged and is not this runner's target.

The source is `infra/scripts/managed_runtime_negatives.py`; local tests are `infra/test/managed_runtime_negatives_test.py`. `author-checks.json` binds their exact hashes. Fourteen tests pass both on the host and inside the existing Linux ARM64 image with networking disabled. The container, temporary test files and ephemeral sockets were removed. The Linux check independently verifies that a bound, non-listening loopback5432 socket produces `ConnectionRefusedError`. Unit sequencing uses synthetic credentials/helper/native fixtures and an ephemeral port; these checks do not establish managed refusals or native managed readiness. The unselected local marker-tolerance experiment is retained in `unselected-marker-tolerance-checks.json`; root retained the original strict parser after independent exact-image evidence confirmed its first-line contract. That experiment is not the registered candidate.

## Exact one-off interface

- Use the current reviewed application task's roles, private networking, UID65532, read-only filesystem and private `/run/genui` volume. Root must bind the registered task to source `8f8c34ba0b5e47a55990b273fc6d555d41369791`, image `sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22` (ARM64), with the existing nonsecret managed runtime configuration. No application service update is part of this operation.
- Set only the explicit qualification opt-in `GENUI_QUALIFY_SYNTHETIC=APP-007`; do not inject any `GENUI_AUTH_QUALIFICATION_*` variables. The runner refuses even empty auth configuration variables. No password secret read, auth flow, LLM request, grant change, migration, persistent object write or wrong-tenant/key trial is invoked.
- Register entryPoint `["python3", "-c"]`, command `[<complete reviewed runner source>]`. Do not run via the normal `runtime.py serve` supervisor or write the source into an existing credential path. The runner has no CLI-selected path, case, tenant, role, helper or endpoint. Its one native child is always `/app/genui`, using `/app/runtime.py` and the current interpreter, bound to `127.0.0.1:0`.
- The runner verifies `/app/runtime.py` SHA256 `83df42285e43af81118b055a29a0b2f2e99aea1d37d6a7ead5fa7354c38a84d4` before importing it. Root's task/image receipt supplies binary identity; the runtime hash does not independently attest the whole image. Neither pending runtime edits nor the newer local callback image is accepted by this runner.
- A single sanitized JSON report is printed; success requires exit0 and `passed:true`, all five fixed checks passed, four negative503/recovery200 pairs, `private_files_removed:true`, and `native_process_stopped:true`. Retain the source hash, exact task definition/image identity and stopped exit status separately. Output includes fixed labels, booleans, counts, HTTP statuses and elapsed milliseconds; it includes no host, port, path, token, credential generation/digest, provider URL, secret, native log or raw exception.

## What an actual successful run would prove

The runner first checks UID65532 and an owned0700 `/run/genui`, creates a new owned0700 temporary directory there, and invokes the unchanged **real** `runtime.refresh` into a new private baseline file. That function assumes the four fixed database and two storage roles and generates temporary IAM credentials; it does not read the database password secret. A0600 private active copy is checked by the real `runtime.probe`, which performs four verified-TLS database identity/membership/schema reads, two own-prefix S3 listings and two own-key KMS encrypt/decrypt round trips. The runner then starts the real native binary and waits for its own helper-backed `/health/ready`200.

For each case, only the isolated active copy changes. The baseline and any shared service credential file are never mutated:

| Fixed case | Controlled change | Exact helper expectation | Native expectation |
|---|---|---|---|
| `missing_file` | Remove the owned active file | `FileNotFoundError`, with file still absent |503|
| `wrong_storage_region` | Change only `storage.region` to `us-west-2` | `runtime.Refused` during local validation |503|
| `expired_file` | Set only top-level `expires_at` to `issued_at - 1` | `runtime.Refused` during expiry validation |503|
| `unreachable_database` | Change only DB host to `127.0.0.1`; preserve port5432 and all original credentials | Real `psycopg.OperationalError`, no SQLSTATE, matching loopback connection refusal |503|

The unreachable case first reserves loopback5432 without listening. If that port is occupied, the trial fails before modifying the file or contacting that listener. The original credentials cannot reach an unrelated loopback service. The first failed database connection stops this helper invocation; this case is an unreachable **database** test, not an S3/KMS outage test.

After every successful refusal, the exact baseline is atomically restored; another real helper probe must pass, the baseline file must still match, and native readiness must return200. Thus a healthy foundation is established before negatives and restored after each one. The helper performs read-only SQL and storage listings; KMS operations process only the existing fixed synthetic readiness plaintext. No persistent managed data is changed.

Native readiness binds its cache to the exact credential-file digest, generation and expiry. A changed digest can produce503 before the native background helper attempts the failing operation. The receipt therefore records the **separate direct real-helper refusal** as well as the real native503. A restored native200 may reuse its still-valid cache; the separate direct helper positive after each restoration establishes fresh managed connectivity. This is not a claim that every native background cycle was individually observed.

## Bounds and failure cleanup

The work alarm is110 seconds and final cleanup receives at most10 additional seconds. The native startup-marker parser reads at most4096 bytes with a5-second bound; HTTP connects directly to loopback with a1-second timeout, accepts only200/503 and an empty body, and never follows redirects. The native implementation bounds its helper at25 seconds and retries after15 seconds; each readiness wait is27 seconds, subordinate to the whole-run alarm. Slow/unavailable dependencies fail the trial rather than extending its window.

The active copy is restored in a `finally` path even after a failed case or interruption. The native process group includes readiness helper children; cleanup sends TERM, waits at most3 seconds for the native leader, sends KILL to the group and waits at most2 seconds for the leader. The owned temporary directory is then removed. A process crash/SIGKILL cannot guarantee Python `finally`; the actual stopped-task and ephemeral-volume lifecycle remains root's cleanup responsibility. The receipt's native-stop flag refers to the reaped leader after group termination, not a separate census of every kernel descendant. The directory is trusted to the same UID, matching the existing runtime boundary; this is not a hostile same-UID filesystem race defense.

Wrong-region and expiry cases intentionally refuse before networking. They are actual invocations on a freshly obtained managed bundle when root runs them, not AWS wrong-region denials or natural credential expiry observations. The changed expiry does not revoke any IAM credential. Existing eleven-check wrong-tenant storage/key evidence is retained and not repeated.

Local command: `python3 -B -m unittest discover -s infra/test -p managed_runtime_negatives_test.py -v`. The additional recorded image run used the exact existing8f image with `--network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges`,2CPU/1GiB, bounded temporary filesystems and no credentials. No application/runtime source, image, ticket, workflow, managed resource, Git index or commit was changed by this author task.
