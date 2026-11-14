# APP-197: how to repeat the 26-page release measurement (recipe)

This file says how to run the 26-page `refresh-herd` measurement once, so that it can be reproduced exactly. Run it on release B's integration branch, which carries APP-195, APP-197 and W4's APP-200 (24 snapshot slots), and run it on a quiet host. It is a recipe, not a result. Record the result in a new, dated file next to this one.

## The host must be quiet

Before starting, check and write down all of the following.

- `docker ps --format '{{.Names}}'` shows no `genui-native-*` container. Every campaign starts its own; the measurement starts exactly one of its own.
- No other worktree is building or running a campaign: `ps -Ao pcpu,comm -r | head` shows no `rustc`, `cargo`, `clippy-driver` or `genui-*` test binary.
- `uptime`: the 1-minute load average is below 4 on this 10-core Mac.
- Docker's VM: `docker info --format '{{.NCPU}} CPUs, {{.MemTotal}} bytes'` (it was 10 CPUs and 8.3 GB on 2026-11-14).

If any of these fails, wait and check again. Do not run it on a loaded host: on 2026-11-14 the same code gave p95 values of 5–6 s at load about 10 and 8–26 s at load 10–22.

## The checkout

```sh
git rev-parse HEAD            # record it: the integration branch head
just setup                    # venvs and node_modules for this worktree
npm --prefix app/admin run build   # RustEmbed needs app/admin/dist
```

## The command

Build first, so that compiling does not overlap the run. Then run the same test binary.

```sh
cargo test --release --locked -p forma-app-platform --lib --no-run

uptime; docker ps --format '{{.Names}}'
GENUI_REFRESH_HERD_FULL=1 \
GENUI_QUERY_PYTHON="$PWD/app/runtime/.venv/bin/python" \
GENUI_REFRESH_HERD_REPORT="$PWD/.cache/reports/refresh-herd-release.json" \
cargo test --release --locked -p forma-app-platform --lib \
  storage::qualification::refresh_herd::native_refresh_herd_campaign \
  -- --ignored --exact --nocapture 2>&1 | tee .cache/reports/refresh-herd-release.log
uptime
```

- It takes about 6–8 minutes after the build.
- **Exit status.** The test asserts only the 15-page gate (run A, every criterion) and gate C (at least 10 × 503). A failing exit therefore means the gate failed. The 26-page part is recorded, not asserted.
- `GENUI_REFRESH_HERD_FULL` must be exactly `1`.

## What is fixed in the campaign

These are not settings; they live in `app/src/storage/qualification.rs`, module `refresh_herd`.

- A Tokio multi-thread runtime with 4 worker threads, inside a 16 MiB-stack thread.
- 1 owner and 25 members; each page reads the document, the record-components batch and the typed view at once.
- Three herds, 10 s apart. The client spread is 0–3 s, the retry waits 1–3 s, and the view retries a 429 at most three times after `Retry-After`.
- Seeds:
  - the gate uses 1 (A) and 2 (C);
  - the full measurement uses 11 (A), 12 (B) and 13 (C);
  - the page-count and cadence searches use 14 and up.
- Run A's criteria:
  - no failed read after its retry;
  - at least 90% first-try 200;
  - p95 within 6 s and every page within 10 s;
  - every hint delivered;
  - the owner's command under 1.5 s.
- **The release measurement's own verdict** (`measurement_26_pages` in the report) is run A at 26 pages with:
  - `no_terminal_failure`;
  - `every_page_current_within_10_s`;
  - `p95_current_within_6_s`;
  - `command_under_1500_ms`.

  It also reports the first-try share per herd, the catalog checks and the data-role transaction service p50.

## What to record

- The head SHA.
- The two `uptime` lines, the `docker ps` output and the Docker VM line.
- From `.cache/reports/refresh-herd-release.json`:
  - `measurement_26_pages`;
  - `gate_passed` and `gate_c_503`;
  - `costs_alone`, which holds the data-role transaction alone and the catalog checks in 20 transactions;
  - for each herd of the 26-page run A: `first_try_200_share`, `current_p95_ms`, `current_after_send.max_ms`, `terminal_failures`, `view_429_answers` and `commit`;
  - `admission_saturated` and `transaction_service` for that run;
  - `largest_passing_pages` and `sustainable_cadence`.
- Copy the report and the log into `evidence/APP-197/`, with the date in their names.
