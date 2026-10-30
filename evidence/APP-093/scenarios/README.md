# ADR-011 scenarios: action path campaign and two-window shoot (local stand)

Synthetic identities and data only (the Northstar fixture organization as the demo organization; personas "Diego Ramos", "Sarah Chen", "Priya Natarajan" are seeded names). No credential, address or cookie value is retained.

- `actions-campaign.json`: `storage::actions_campaign::native_actions_campaign` (Docker PostgreSQL, the operator's `seed-library`, a loopback Messages stub through `GENUI_COMPOSER_ENDPOINT`): 10 checks, `passed: true`, with the per-step timings (`act_create_ms` 704, `confirm_create_ms` 200, `share_ms` 110, `sarah_app_shared_event_ms` 43, `act_decide_ms` 772, `confirm_decide_ms` 307, `diego_record_committed_event_ms` 44, `act_model_ms` 431).
- `two-window-shoot-notes.json`: the Playwright run against the stand (`GENUI_ACT_STAND_SECONDS`), two Chrome windows (employee, approver): confirmation card in 790 ms from Enter, Confirm to "Filed" in 366 ms, the approver's canvas showed the pending row live, the employee's row turned approved 423 ms after her Confirm. The screenshots (`01`–`11`) are kept in the session scratch (`pitch-scenarios/`), not here.
- The real Haiku parse smoke (five utterances, decisions and latencies only) is recorded in the APP-093 ticket log.

Procedure: `docs/pitch/SCENARIOS.md`.
