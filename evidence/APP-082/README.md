# APP-082 — exact-source CI receipt

This file records the tenant-foundation run. The independent review that binds it is [independent-review.md](independent-review.md). Ticket: [APP-082](../../Tickets/3-in-review/APP-082.md).

Exact source: `a9a730531dc6644d2fa6ef291a967080886f596a` (`origin/main`).

Workflow: `.github/workflows/tenant-foundation.yml`, step "Qualify route-level invitation races and provider faults (APP-082)".

```text
GENUI_INVITE_RACE_REPORT="$RUNNER_TEMP/tenant-foundation/invite-races.json" cargo test --locked -p forma-app-platform --lib auth::invite_tests::native_invite_race_campaign -- --ignored --exact --nocapture
```

Run: https://github.com/getforma-dev/forma-app-platform/actions/runs/41955377535

- Event: push to `main`
- Head: `a9a730531dc6644d2fa6ef291a967080886f596a`
- Job: `tenant-foundation`, id `127008916230`, conclusion success
- Step 16 started `2026-11-05T02:45:29Z`, completed `2026-11-05T02:46:41Z`, conclusion success
- Log: `test auth::invite_tests::native_invite_race_campaign ... ok`
- Log: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 430 filtered out; finished in 71.72s`

Artifact `app-008-tenant-foundation-evidence`, file `invite-races.json`, sha256 `8574a23e396075e6a4bc95d654c8bccd3018f6e04089799452fd279f480446d4`. Copy: [ci-run-41955377535/invite-races.json](ci-run-41955377535/invite-races.json).

The CI report has `passed: true`, 12 checks, every check `passed: true`, `hosted_acceptance: false`, `managed_acceptance: false`, PostgreSQL `16.15 (Debian 16.15-1.pgdg13+2)`.

[race-campaign.json](race-campaign.json) is the earlier local run, sha256 `0dd52616abae028c016ab2673e21554113021458f69d6326d3cdee4e19bbfe07`. Its check names, pass flags, dispatch counts and limits match the CI report. Its `server_version` is `16.13 (Debian 16.13-1.pgdg13+1)`.

Prepared by Grok.
