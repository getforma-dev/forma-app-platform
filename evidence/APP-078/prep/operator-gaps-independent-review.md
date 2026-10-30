# Independent Tier A review: operator gaps merge (`ce25ddf` + `3012c2d`)

Reviewed 2026-10-30 on a `git archive 3012c2d` copy, read-only against the checkout. Scope: the merge `ce25ddf`
(branch commits `d3f3185`, `21df7c1`, diffed against `ce25ddf^1`) plus `3012c2d`: APP-076 gaps G2 (managed
invitations), G3 (Spend Review seeded into Northstar), G5 (redeemer ids in `list`), G11 (deny-all rollback revision),
B1 review minors m1/m2/m3/m5/m6, and the `trial_checks.py` defaults. No AWS, DNS, fAuth or mail call was made; no
container of another session was touched (the suites start and remove their own `genui-app073-operator-*` containers).

## Verdict: no blocking findings

Seven minors, none of which makes the window unsafe. One (M1) refutes a claim the merge states in a code comment and
is a one-line fix with probe strings; two (M2, M3) are runbook corrections for 15.3; the rest are observations with a
suggested change. Everything the merge asserts about secret custody, argv/CloudTrail/log hygiene, the one-create
invariant, the seed's idempotence and the deny-all rollback held up to reading and to the probes below.

## What ran (all on the archive copy under the session scratchpad)

| Suite | Result |
| --- | --- |
| `app/runtime` unittests (`test_admission_operator`, `test_provider_invitations`, `test_runtime`, `test_qualification_supervisor`, `test_query_worker`; op-python with jsonschema and duckdb; Docker PostgreSQL 16.13 per class) | **157 passed**, 0 skipped, 151 s |
| `app/operator/test_operator.py` in a fresh venv from `app/operator/requirements.lock` (`--require-hashes --only-binary=:all:`), `GENUI_OPERATOR_TEST_PARSER` = the checkout's `parser/target` | **38 passed** once the archive copy tracked `parser/vendor/zip-9.1.3/Cargo.lock` as the real repository does (the vendored crate's own `.gitignore` hides it from `git add -A`; `test_dockerignore_is_an_exact_allowlist_of_tracked_files` compares the allowlist with `git ls-files`, so it failed on the first run for that copy-artefact reason only). Without the parser: 32 passed, 6 skipped. Note: `python -m unittest` *from inside* `app/operator` cannot run at all because `operator.py` shadows the stdlib `operator` module (circular import in `enum`); run it as a script with `-I` or `discover -s app/operator` from the root as the runbook does. |
| `infra`: `npm ci`, `npm run typecheck`, `npm test` | typecheck clean; **29 of 29** |
| `infra/test/*_test.py` (`python3.11 -I`) | managed_runtime_negatives 14, preflight 29, readonly_snapshot 2, window_requests 11, all OK |
| `cargo +1.93.1 test --locked --lib storage::managed_qualification` | **20 passed** (3 ignored); `SEED_INPUTS` pin `d2f8a6ea…` equals `sha256(app/runtime/tenant_seed.py)` at `3012c2d` (recomputed independently) |
| `native_admission_campaign` (`GENUI_NATIVE_FIXTURE_PYTHON` = op-python, `--ignored --exact --nocapture`) | **passed**, 21.75 s; every APP008 check line printed, including item 7 (loopback fAuth stand-in: revoked/expired/exhausted codes gain nothing, reconcile confirms every remote revocation without a second create) and "no issued code and no provider invitation token appears in a full database dump" |

Own probes: `runtime._exact_origin` and `runtime._VALID_ID` against hostile strings, cross-checked with a WHATWG URL
parser (Node 22 `new URL(...).origin`; the service's `url` crate 2.5.8 is the Servo WHATWG implementation and
`auth/mod.rs:101-108` compares `origin().ascii_serialization()` with the value exactly as the Python does); the seed
pin; the fixture bundles' principals; the Rust session path for the rollback question.

## Findings

### M1 (minor) — `_exact_origin` still accepts hosts the service's `url` crate normalizes or refuses

`app/runtime/runtime.py:339-340` (`_ORIGIN_HOST`) and `:348-365` (`_exact_origin`). The B1 m1 fix refuses a space,
`%`, uppercase and over-long values, and the comment claims "a value this reader accepts is one the service's
decoders accept too", "so nothing this reader accepts can make the child refuse its configuration file and crash-loop
the task". The DNS alternative of `_ORIGIN_HOST` does not apply WHATWG's "ends in a number" rule, under which a host
whose last label is numeric (decimal, `0x` hex, or leading-zero octal) is parsed as IPv4 and either normalized or
refused.

Probe (Python reader vs WHATWG origin):

| value | `runtime._exact_origin` | WHATWG (`url` crate behaviour) |
| --- | --- | --- |
| `https://0x7f.1` | True | serializes `https://127.0.0.1` → `exact_origin` false → **config refused** |
| `https://127.1` | True | `https://127.0.0.1` → refused |
| `https://2130706433` | True | `https://127.0.0.1` → refused |
| `https://1.2.3` | True | `https://1.2.0.3` → refused |
| `https://012.0.0.1` | True | `https://10.0.0.1` → refused |
| `https://example.0`, `https://1.2.3.4.5`, `https://09.1.1.1` | True | parse failure → refused |
| `https://exa mple.com`, `https://ex%41mple.com`, `https://EXAMPLE.com`, `https://a_b.com`, `https://[::1]` | False | (refusing more than the service is fine) |

Consequence: a composed `AuthConfiguration` whose `issuer_origin` has a numeric last label passes
`window_requests.py auth-configuration` and `serve()`'s validation, is written into the files, and the child refuses
its configuration at start (`AuthConfig::load` → `refuse`), which is exactly the crash-loop m1 set out to exclude.
Impact on the window is nil (FormaStack's `issuer_origin` is a DNS name and the operator composes it), so this is a
claim defect, not a risk.

Fix (exact): in `_exact_origin`, after `url.hostname` matches, apply the WHATWG rule — when the last label (ignoring a
trailing dot) matches `[0-9]+|0[xX][0-9a-fA-F]*`, accept the host only if the whole host matches the strict dotted-quad
alternative; otherwise return False. For example:

```python
labels = url.hostname.rstrip(".").split(".")
if re.fullmatch(r"[0-9]+|0x[0-9a-f]*", labels[-1]) and _IPV4.fullmatch(url.hostname) is None:
    return False
```

(with `_IPV4` the existing dotted-quad alternative split out). Add `https://0x7f.1`, `https://127.1`,
`https://1.2.3`, `https://example.0`, `https://012.0.0.1` to the B1 m1 probe list in `app/runtime/test_runtime.py:1034-1041`.

### M2 (minor) — runbook 0.6 invites a `provider_invitation_exists` refusal at 15.3 that 15.3.4 does not list

`evidence/APP-078/prep/RUNBOOK.md:44` suggests the `spare` recipient may carry "Victor's address or the spare's".
`admission_operator.py:764-775` (`issue`, unchanged by the merge and reached by the managed form through
`bind_invitation_input`) refuses a second server-only create for the same `(issuer, tenant, email digest)` while a
code with a non-failed create is active, unexpired and unrevoked, because fAuth's create revokes the earlier pending
invitation for the same address. With both labels on Victor's address the second `issue-<label>` step refuses
`provider_invitation_exists` before writing anything; 15.3.4 (`RUNBOOK.md:163`) names `invitation_input_missing`,
`provider_configuration_missing`, `provider_tenant_mismatch` and `failed/unknown` but not this one, so the operator
would be reading an unexpected refusal under time pressure.

Fix: in 0.6 require a distinct address per recipient (or state that a same-address spare can only be issued after the
first code is revoked or expired), and add `provider_invitation_exists` with that meaning to 15.3.4's refusal list.

### M3 (minor) — "emptied" is not "erased": the address and the key stay readable as `AWSPREVIOUS`

`RUNBOOK.md:162-164` and ADR-003 amendment (`docs/architecture/ADR-003-PUBLIC-BETA-ACCESS.md:49`) describe the input
secret as "overwritten with `{}`" and 15.3.3 says the address then "exists in exactly two places". A
`PutSecretValue` does not overwrite: Secrets Manager moves the prior version to the `AWSPREVIOUS` stage, where
`get-secret-value --version-stage AWSPREVIOUS` (or `--version-id`) returns the address-bearing document to any holder
of `GetSecretValue` on the secret (the operator host's credentials; the `MigrationExecutionRole`). The same holds for
FormaStack's A12 reset of `ProviderManagementKey` to `{}`: the management key remains the `AWSPREVIOUS` version until
another put moves it out of the labeled stages (unlabeled versions are deprecated and deleted by Secrets Manager
later, not immediately). APP-084 item 7 (`Tickets/0-backlog/APP-084.md:36`) records rotation and audit but not this.

Fix: at 15.3.5, after the `{}` put, perform a second `{}` put with a new `--client-request-token` (or
`update-secret-version-stage --move-to-version-id <the {} version> --remove-from-version-id <the address version>
--version-stage AWSPREVIOUS`) and confirm with `describe-secret` that no version in `VersionIdsToStages` carrying a
stage holds an address; ask FormaStack to do the same two-put reset for A12; note in APP-084 item 7 that deprecated
versions linger until the service deletes them. No code change.

### M4 (minor) — `bound_elsewhere` only visits organizations that already hold a handle or a code

`admission_operator.py:707-726`. The m6 check unions `genui_private.handle_index` and `genui_private.code_index` for
its candidates. An organization created by `provision` (which also writes its binding) that has not yet been issued a
handle or a code is skipped, so the same `(issuer, tenant)` can be bound to it and to Northstar; it becomes
ambiguous the moment a handle is issued to it, and nothing at that later point re-checks. Routing authority is the
registry (one binding per routed organization since m2), so the service is not confused by the duplicate database
row, but the check's name promises more than its candidate set delivers.

Fix: APP-084 item 7 already contemplates a `genui_private` definer helper; until it lands, document the candidate set
in the refusal's docstring and the override's purpose text, or include `genui.organizations` reachable through the
existing owner-visible `genui_private` indexes. Not for this window.

### M5 (minor) — `list` redemptions use INNER JOINs

`admission_operator.py:857-862`. The G5 query joins `code_redemptions` to `memberships` and `entitlements`; a
redemption whose membership or entitlement row were ever absent would silently disappear from `redemptions` rather
than show with a null status. `admit_redemption` writes the three together, so this is robustness only.

Fix: `LEFT JOIN` both and emit `null` statuses; one-line change, same test.

### M6 (minor) — the two injected secrets reach every operator subcommand's process and its children

`infra/src/stack.ts:410-414` injects `GENUI_PROVIDER_MANAGEMENT_JSON` and `GENUI_OPERATOR_INVITATION_INPUT_JSON` into
the operator container for every override, including `migrate`, `seed`, `verify-catalog` and `upload-sources`.
`app/operator/operator.py:317` runs the parser-host subprocess with the inherited environment. The parser host is
the reviewed Rust binary and nothing in the operator prints `os.environ`, so this is exposure surface, not a leak.

Fix: pass a minimal `env=` to the parser-host `subprocess.run`, and (later, APP-084) consider popping the two
variables in `operator.py main()` for every command but `admission`.

### M7 (minor) — A12's write needs IAM and KMS rights the stack does not grant to any external principal

`infra/src/stack.ts:380-394` creates `ProviderManagementKeyKey` with CDK's default key policy (account root only) and
grants `kms:Decrypt` through Secrets Manager to the `MigrationExecutionRole` alone; nothing grants
`secretsmanager:PutSecretValue` or `kms:GenerateDataKey` to FormaStack. 5.9a therefore succeeds only if FormaStack's
"own credentials" are an administrator principal of the same AWS account; from another account, or from a scoped
same-account user, the put fails `AccessDenied` and the window stops at 5.9a. The runbook does not state which.

Fix: confirm the principal in 0.1 (A12); if it is not a same-account administrator, add a resource policy on the secret
and a key-policy statement for that principal (PutSecretValue, GenerateDataKey via Secrets Manager) in `stack.ts` and
assert them in `security.test.ts`.

## Claims I failed to refute

**G2, managed invitations.**
- *Who can read the management key or the invitee input.* `security.test.ts` (29/29) asserts: the two secrets appear only in the `OperatorTask` container's `Secrets`; their KMS keys decrypt only for `MigrationExecutionRole` via `kms:ViaService`; the only IAM grant is that execution role's `GetSecretValue`/`DescribeSecret`; the service's execution role still reads exactly its two secrets; `ExecutionRole`, `TaskRole` and `OperatorRole` name neither secret nor key. The service, migration, qualification and rollback task definitions do not name them. The value reaches the operator process as an environment variable (the task role needs no grant), which is what the design states.
- *Argv, overrides, CloudTrail RunTask parameters, logs, evidence.* `admission-issue-managed-invitation` (`overrides.json`) carries the digest, grants, `--issued-by` and the bare flag `--provider-input-from-secret`; `window_requests.compose_operator_command` replaces only documented flag values; `operator.managed_form_refused` refuses any `PROVIDER_FLAGS` in argv in a managed task (test asserts the address is absent from the refusal). `issue`'s output keys are `code_id, code_digest, kind, max_redemptions, expires_at, provider_invitation{operation,state,capability_stored,detail,invitation_id,remote_invitation_id}`; `test_managed_invitation_takes_the_address_from_the_input_secret_and_prints_identifiers_only` asserts the address (both cases), the key, the raw code and the `invite_token` are absent from stdout, stderr and a full `pg_dump`. Refusals are fixed reason codes (`_unique_object`, `invitation_input`, `management_configuration` never echo a value; `main` maps any other exception to `admission_operator_refused`). `window_requests.py invitation-input` prints labels, organizations, digests and the file's digest only (asserted by `window_requests_test.py`).
- *Entry bound to exactly one digest and organization.* `invitation_input` (`:483-511`) requires the closed key set, string fields, `identifier()` on the organization, `DIGEST` on the digest, `check_invitation` on the fields and a unique `(organization_id, code_digest)` per document (≤64 entries, ≤64 KiB); `bind_invitation_input` (`:514-525`) requires exactly one entry equal on both the organization *and* the digest given in argv, before any connection. The composer binds the digest from the minted file by label and checks the minted file's organization equals the recipient's. In the database, `genui_private.code_index(code_digest)` refuses a second code for the same digest (asserted: "a reused digest refuses whole: no second code, no second create"), `lock_binding` requires an active `(issuer, tenant)` binding for the organization, and `Client.require_tenant` checks the key's tenant (`GET /manage/config`) against the entry's tenant before any lock or write.
- *One create, retried or stopped mid-run.* The managed form runs the pre-existing path unchanged: `pending` row in the code's transaction, one `create_remote` after COMMIT with `FOR UPDATE SKIP LOCKED` and a recorded send deadline, outcome recorded or left to `reconcile`. An ECS retry of the same RunTask (same `clientToken`) is idempotent at ECS; a re-run with the same digest refuses on `code_index` before any provider call; a task stopped between COMMIT and the create leaves a `pending` row that `judge_creates` settles by inspection within the deadline window; stopped after the create, `outcome_unrecorded`/`unknown` is reconciled, never re-created (the native campaign's item 7 exercises confirm-without-second-create).
- *Input secret left populated.* Only `issue --provider-input-from-secret` reads `GENUI_OPERATOR_INVITATION_INPUT_JSON`; every other command ignores it; a repeat of the same issue refuses on the digest. The residual is M3 (version history), not a second create.
- *Reconcile/revoke order in the managed task.* `revoke` (`:883-931`): local status change and `revoke` rows committed first; capability removed from the secret second (`destroy_code_capabilities`, with `{}` management it still completes, asserted `capability_phase: completed, provider_phase: not_configured`); remote revocation last. `_reconcile`: 1 propagate expiry/exhaustion/revocation rows, 2 `sweep_capabilities` before any provider call, 3-6 inventory, creates, remote endings, revocations. ADR-003's order is kept; the merge changed only where the configuration comes from (`provider_management.load_config:199-216`: injected JSON is the sole source, a plain variable beside it is `provider_configuration_conflict`, `{}` is `provider_configuration_missing` for required callers).

**G3, Spend Review seed.**
- `seed_spend_review` (`:1227-1268`): `begin` (identity, catalog proof, `genui_owner`, scope) → organization `FOR SHARE` (D7.1) → `tenant_seed.seed([make_bundle("spend-review", org)], changes)` in the same transaction → D7.7 check → COMMIT. `make_bundle` refuses any organization outside `ORGANIZATIONS` and any fixture outside `FIXTURE_NAMES`, rewrites the retained `impl-2026-10-28.1` bundle exactly as `make_bundles` does for `…0002`, and validates it with the contract validator. `seed`'s `INSERT … ON CONFLICT DO NOTHING` followed by `ROW(...) IS NOT DISTINCT FROM ROW(...)` makes a repeat `changed: false, rows_inserted: 0` (asserted on a dump) and makes any differing existing row raise, which becomes `seed_refused` before COMMIT and is rolled back by `_close`; nothing is updated, ever. The Canary organization is untouched (asserted `rows(canary)` unchanged). D7.7: `commit_sequence_mismatch` when a backfilled `organization_sequences` row is not ahead of the highest seeded revision, checked before COMMIT so nothing is kept. The seeded `provider_tenant_bindings`/`organizations` rows the bundle shares with the original Northstar seed are exact repeats. Honest limitation stated in the docstring: the definition is not re-admitted through the Rust whole-output path; the rows are the seed's own, same bytes and digests (asserted against `canonical(content)` and the fixture digest).
- Note, not a finding: the bundle also adds its synthetic actors (`…00ca`, approver `…00d1`) as members with entitlements and permissions in Northstar. They are `app008-fixture-only` identities; the live registry binds the real issuer only, so no real sign-in can resolve to them (the session path requires `registry.agrees(issuer, tenant, organization)`).
- ADR-010 §1 row 3 labels seeded definitions "seeded definition, retrieved from the library"; the command returns `label: "seeded definition"`.

**G5.** `list_codes` emits `redemption_id, principal_id, entitlement_id, redeemed_at, membership_status, entitlement_status` only; the test asserts the provider subject and the raw code are absent from the whole listing.

**G11, rollback.**
- `RollbackTask` (`stack.ts:228-239`) is the service task definition with the same image parameter, roles, environment, volume, port and health check and no `secrets`; `security.test.ts` compares it field by field and asserts the ECS service never references it; `preflight.activate` (`preflight.py:263-277`) requires its ARN ≠ the live one, the reserved image digest and architecture, one `workspace` container, no secrets, environment files, entrypoint or command, and no `GENUI_AUTH_QUALIFICATION_*`/`GENUI_PROVIDER_CAPABILIT*`/`GENUI_AUTH_CONFIG*`/`GENUI_IDENTITY_REGISTRY*` variable.
- *Truly deny-all, including an existing session.* With neither secret injected `serve()` (`runtime.py:904-935`) hands the child `EmptyIdentityRegistryFile` and no `GENUI_AUTH_CONFIG_FILE`/`GENUI_PROVIDER_CAPABILITY_DIR`. In the service, `main.rs:400-412` treats the absent auth config as "sign-in fails neutrally"; `auth::invite::Invitations::from_env()` without the directory means no code obtains a handoff; and every protected request is fenced on the registry: `storage/sessions.rs:757+` (`resolve_session`) requires `s.registry_sha256 == registry.identity.sha256 && registry.agrees(issuer, tenant, organization)`, and `storage/authorization.rs:181-186` repeats both for every `TrustedRequest`. The empty registry's digest differs from the candidate's and `agrees` is false for every binding, so a session cookie minted by the candidate is `Denied` (uniform 404) on the rollback revision: the claim holds for live sessions too, not only for the outsider matrix the local evidence ran. No write path exists on that revision that could restore a revoked entitlement; the rollback changes no row.
- *Restore path.* `update-service --task-definition <candidate>`; the stack's `Service` keeps the candidate as its configured task definition, so CloudFormation drift resolves on the restore. The local evidence's stated limitation (the registry-bearing half of the start not run on the host) is accurate and is covered by the code paths above and `test_runtime.py`.

**B1 minors.**
- m1: `_VALID_ID` (`runtime.py:336`) is `_CAPABILITY_UUID`, byte-for-byte the predicate of `storage::valid_id` (lowercase hex, version nibble 4 or 7, variant 8-b); probe: v1 and variant-c ids and uppercase refused, v4/v7 accepted. The host half is M1.
- m2: `auth_configuration` requires the binding organizations to equal the routed set and `len(bindings) == len(routed)`: exactly one per organization.
- m5: `bind-provider-tenant --from-auth-configuration` reads the secret's ARN from `GENUI_AUTH_CONFIGURATION_SECRET_ID` (added to the operator environment, asserted in `security.test.ts`) or `--auth-configuration-secret-id`, validates the whole document with `runtime.auth_configuration`, takes the one binding for the organization, and prints `source: auth_configuration` with no issuer or `ten_` id (asserted). Mixing the two forms is `binding_source_refused`.
- m6: `provider_tenant_bound_elsewhere` refuses a pair active for another organization that can sign in (test with a second organization holding a handle); both halves of the pair must match. Candidate set: M4.
- m3: ADR-007 D7.8 carries the dated recorded exception naming `bind-provider-tenant`, `register-source-revision`, `create-bound-work`, `seed-spend-review` and the provider-invitation operations; APP-084 items 6 and 7 hold the follow-ups.

**Trial script defaults (`3012c2d`).** `trial_checks.py:738-741` points the "foreign" rows at Northstar's app `…0065`, the seeded work `…0069`, collection `…006a` and a record id with no row. A member run with defaults cannot commit for three independent reasons: the NS code grants `record_read` on the bound work `…00f2`, never a write on `…0069` (APPROVALS A9); `command_body` carries `expected_work_version: 999999`/`expected_record_version: 0`, a premise no saved work holds (412 before any write); and its hard-coded `publication_id …00cc` is Spend Review's, not Northstar's. The member context sends no `command_known` row (`ctx != "member"`), and the controls are reads.

**Managed-task guard.** `operator.py:102-127`: raw `issue` and `issue` with any `PROVIDER_FLAGS` refuse; `register-source-revision --record` refuses; `issue --code-digest [--provider-input-from-secret]`, `register-source-revision --record-json`, `reconcile`, `revoke` and `seed-spend-review` pass to `admission_operator.main`, which refuses again before connecting when the configuration or input is missing (asserted for `reconcile` and the managed `issue` form inside and outside a simulated task).

## Fix priority

Before the window: M2 and M3 are runbook text (15.3 and 0.6) and cost nothing; M7 is one question to FormaStack at 0.1.
With the next code pass: M1 (with its probe strings), M5, M6. Later (APP-084 item 7): M4.
