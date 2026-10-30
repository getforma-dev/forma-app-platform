# APP-076 go-live trial — 2026-10-30 (in progress)

Run against `https://beta.getforma.dev` on the APP-078 candidate (`aa220b3`, service revision `GenuiSyntheticFoundationTaskE2459512:4`) under Victor's approvals T1–T9. Victor took part from his phone and laptop; his observations are quoted in the private trial record. No address, code, cookie, token or full callback URL is recorded here.

| Item | Status | Evidence |
| --- | --- | --- |
| 3 Invite → verified sign-up → sign-in (installed Chrome, real mail) | **Passed twice.** NS code: `WRONG` refused (12:02:28 `invite.validate outcome="refused"`), the code handed off (12:03:24), fAuth sign-up and SES mail verification, sign-in; the queries below ran in that session. Phone code (second address, phone browser end to end): handoff 12:35:38, sign-up, verification, sign-in, queries at 12:39. | service log lines; Victor's confirmation |
| 6 (query part) Authorized query with receipt | **Passed** on laptop and phone: `total` and `timeline` dispatched in the confined child and released with receipts (`query.released`); Victor saw **1,140,000** and, with destination `198.51.100.7`, **900,000**. | service log; Victor |
| 6 (command part) S05 record sequence, restart, reopen | **Pending Victor** (record page, the console snippet, `reviewed`, then the orchestrator's restart and his reopen). | — |
| 9 Private-route refusal matrix | **Passed live**: 287 rows, every class uniform (`not_found` 211 × 404, `cross_site` 71 × 403, `callback` 5 × 400). | [matrix-baseline.json](matrix-baseline.json) |
| 9 Canary replay | **Pending**: needs a Canary-organization session. | — |
| 8 Rollback and restore | **Passed**: the deny-all `RollbackTaskDefinition:2` on the same image (12:49–12:52): landing and health 200, sign-in POST a neutral 400 with no provider redirect, workspace/work/record 404, the full matrix uniform (287 rows, 0 failures); restored to `Task:4` (12:53–12:59), healthy, sign-in 303 to fAuth again. | [matrix-rollback.json](matrix-rollback.json) |
| 10 Audit chains on RDS | **Verified** for both organizations after the sign-ins (Northstar admission 7 events, workspace 4, operator 5; Canary only its operator event); catalog version 7 read-only. Insert-only proof beyond `verify` stays with the reviewer (G14). | operator `verify` outputs |
| 5 Revocation at the end (T5) | **Deferred** until after Victor's record part. | — |

The service is left running on revision 4 as the invite-first beta.
