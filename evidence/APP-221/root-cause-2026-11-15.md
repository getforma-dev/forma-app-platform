# APP-221: the flaky sandbox protocol test (2026-11-15)

By Claude (Opus 5.5), on branch `fix/app-221-sandbox-stub` from `main` at `d00b9395`.

## The failure

CI run 43716303532, attempt 1, job `foundation` (ubuntu-24.04), 2026-11-15 13:06 UTC, on PR #144: `imports::sandbox::tests::protocol_and_identity_violations_are_failures` panicked at `app/src/imports/sandbox.rs:710` on the case whose stub is

```sh
/bin/cat > /dev/null
printf '{"status":"parsed"}'
printf '{"status":"parsed","wasm_sha256":"sha256:%s"}\n' 0000 >&2
```

with `left: Crashed`, `right: Failure("parser_identity_mismatch")`. The re-run passed.

## The cause: `ETXTBSY` at `spawn`, not a pipe race

The stubs drain stdin (`/bin/cat > /dev/null`) before they act, so the parent's write of the input cannot meet a closed pipe; and a broken input pipe is classified `ChildClosed`, never `Crashed`. `Crashed` comes from `supervise::run_confined` when `Command::spawn` fails (or the child dies on a signal).

The test executable is multi-threaded, and its other tests spawn children at the same time. `stub_seeing` wrote each stub with `std::fs::write` and then executed it. A child that another test thread forked while this thread held the stub open for writing inherits that descriptor (close-on-exec takes effect only at that child's own `exec`), and until then the kernel refuses to execute the file: `ETXTBSY`, "Text file busy". `run_confined` maps the spawn error to `Killed::Crashed`, and `parse_with` to `ParseRefusal::Crashed`.

Reproduced on Linux (the CI platform; macOS does not enforce `ETXTBSY`, so the loop ran in Docker) with a temporary line in `run_confined` printing the spawn error:

```
=== before iteration 5 FAILED
APP-221 diagnostic (temporary): spawn failed: Text file busy (os error 26) kind=ExecutableFileBusy
  left: Crashed
 right: Failure("parser_identity_mismatch")
=== before iteration 301 FAILED
APP-221 diagnostic (temporary): spawn failed: Text file busy (os error 26) kind=ExecutableFileBusy
  left: Crashed
 right: Failure("parser_failure")
before: iterations=500 pass=498 fail=2 elapsed=21s
```

([log](stress-loop-before-2026-11-15.log)). Two different cases of the same test failed the same way; the case is whichever one the race lands on. The diagnostic line was removed before the fix was measured and is not in the change.

## The fix: the test never holds an executable open for writing

`stub_seeing` now writes the stub through a child process (`/bin/sh -c 'umask 077 && cat > "$1" && chmod 0755 "$1"'`, the script on its stdin) and waits for it. The descriptor that writes the file belongs to that child, which has exited before the test executes the stub, so no descriptor of the test process ever refers to the stub for writing and no concurrently forked child can inherit one.

Production classification is unchanged. The parser host is written once by the image build and never at runtime, so no process holds it open for writing when `run_confined` executes it; users cannot meet this race, and a spawn failure stays `Crashed`.

## The loop

`loop.sh` (scratch, outside the repository): in `rust:1.93.1-bookworm` on Docker (10 CPUs, Linux 7.0.14-linuxkit), `apt-get install stress-ng`, `stress-ng --cpu 10` for the whole loop, then 500 runs of the test executable (`cargo test --locked -p forma-app-platform --lib --no-run`, built in the container) with the filter `imports::sandbox`, so the sibling sandbox tests fork children in parallel with the test under study.

| | Iterations | Passed | Failed |
| --- | --- | --- | --- |
| Before the fix ([log](stress-loop-before-2026-11-15.log)) | 500 | 498 | 2 (both `ETXTBSY` at spawn) |
| After the fix ([log](stress-loop-after-2026-11-15.log)) | 500 | 500 | 0 |

The `exit 1` at the end of each log is the script's own exit status after it kills `stress-ng`, not a test result.

On macOS: `cargo test --locked -p forma-app-platform --lib -- imports::sandbox` passes (12 passed, 1 ignored); the race does not exist there.
