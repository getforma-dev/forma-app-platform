# Optional qualification secret handoff

This local follow-up adds transport for the already-reviewed APP-006 callback configuration. It changes only the Python supervisor and focused runtime tests. It does not change callback policy, create an invitation, supply application membership, enable public signup, or start a hosted rehearsal. No actual secret, cloud API or auth provider was read/called during qualification.

## Exact container interface

- Optional ECS-injected **`GENUI_AUTH_QUALIFICATION_CONFIG_JSON`** contains the entire configuration document as a string. It is not a secret ARN or a path. The owning infrastructure change supplies this variable through its conditional ECS secret reference; it must not supply a simultaneous `GENUI_AUTH_QUALIFICATION_CONFIG_FILE`.
- The runtime accepts this injection only for **`serve`**. It removes the variable from its own environment before validation, credential refresh or child launch. Empty/whitespace-only values, more than8192 UTF-8 bytes, invalid UTF-8 encoding, and any simultaneous explicit file-path variable are terminal refusals. The file-path key conflicts even if its value is empty.
- Rust remains the sole JSON/schema/expiry/auth-policy validator. The transport preserves the exact UTF-8 bytes and does not parse, normalize or silently repair the document. A syntactically malformed document reaches Rust through the private file, is refused before listening, and is cleaned up after child exit.
- The destination is the existing parent of `GENUI_RUNTIME_CONFIG_FILE`, normally **`/run/genui`**, already owned by the process UID (65532 in the image) with mode0700. Injection refuses a missing, linked, wrong-owner or incorrectly permissioned directory rather than creating one. The runtime credential path must be absolute when injection is used.
- The supervisor exclusively creates a new **`.auth-qualification-<random32hex>.json`** file with mode0600. It never chooses a caller-supplied name or replaces an existing path. The child gets only **`GENUI_AUTH_QUALIFICATION_CONFIG_FILE`** pointing to that file. The raw JSON is absent from the child's environment and command line. The supervisor does not add the generated file-path variable to its own environment.
- When JSON injection is absent, the existing behavior is preserved. With neither variable, the callback remains disabled. A separately supplied existing file-path configuration still works and is never treated as a supervisor-owned file or deleted by this handoff.
- Injection in **`refresh`, `probe`, `migrate`, `qualify` or `cleanup`** is consumed and refused before any managed operation. Rust's normal probe helper inherits only the file path, ignores it and continues its existing managed-readiness behavior.

The callback's existing schema, source pins, external tenant/client identifiers, operator credential, invitation capability, run ID and short expiry remain unchanged. See the parent [callback contract](../README.md). This follow-up supersedes that earlier preparation note that the runtime did not yet create the qualification file; the optional interface above now supplies it. No deployment or secret value is created by this source change.

## Ownership and cleanup

The directory is opened without following its final component and held by descriptor. The new file uses exclusive/no-follow creation. Its descriptor remains open and non-inheritable until cleanup, preventing deletion/replacement from recycling its inode number and falsely passing the ownership check. Before launching Rust, the named directory and file must still match the held identities, owner, permissions and single-link file contract.

The supervisor's `finally` block removes only its matching owned file. A replaced regular file, symlink or new hardlink is refused and preserved; it is never deleted as though it were the original. Descriptor cleanup still runs. An error before credential refresh begins does not remove a pre-existing credential file. Existing credential rotation and post-supervision credential cleanup remain in place.

Normal stop first terminates the child and waits at most5 seconds, then kills it and waits at most5 more. Private-file cleanup runs even if child termination fails. Transient managed credential refresh and process-launch failures retain their existing retry behavior; unsafe/replaced qualification state before launch is a terminal refusal. SIGKILL or a host crash cannot execute `finally`; the owning task/volume teardown remains responsible for that case. There is no claim of memory zeroization or persistence across task restart.

All supervisor failure messages remain fixed strings. Raw SDK exceptions, configuration bytes and secret paths are not printed. File cleanup never scans or removes other configuration files.

## Local evidence

Run from the repository root:

```sh
python3 evidence/APP-006/hosted-callback/secret-wiring/qualify.py
```

[author-checks.json](author-checks.json) records the source hashes, command output and binary hash. **22 runtime tests passed:12 new handoff tests and10 existing credential/probe tests.** Checks use real private files, collisions with regular files/symlinks/hardlinks/FIFOs, replacement refusal, transport bounds, default-off and explicit-path preservation, real successful/failed child processes, a real child ignoring SIGTERM, interrupted startup, write failures and every non-serve CLI mode. Only synthetic values are used.

The additional probe runs the real supervisor and existing native Rust binary in three modes: injection absent gives callback404; valid private injection gives the real callback303; malformed JSON is refused by Rust before listening. Only managed credential refresh is replaced with the existing synthetic fixture, and the Rust managed probe is deliberately unavailable. Every owned qualification/credential file is removed afterward; no synthetic secret appears in captured output. No provider request, email, hosted origin, ECS secret retrieval or deployment is qualified here.

Independent review of this runtime slice and the separate infrastructure injection remains a separate receipt. The prior local callback image predates this runtime follow-up; it must not be represented as containing this change. Root owns the combined reviewed commit, image/CI, infrastructure wiring and any later hosted readiness signal.
