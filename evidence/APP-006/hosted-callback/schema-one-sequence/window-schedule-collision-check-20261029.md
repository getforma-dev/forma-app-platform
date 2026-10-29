# Collision check: FormaStack WINDOW-SCHEDULE.md against the GenUI activation runbook

Prepared 2026-10-29 by the GenUI owner (Claude) as a pre-start item (plan Task C1). Inputs: `/Users/victorvillacis/dev/Forma/work/FORMA-247/rehearsal/WINDOW-SCHEDULE.md` (FormaStack proposal, phases P0–P12 relative to T0, cutoff T0+4:30, deadline T0+6:00) and [activation-runbook.md](activation-runbook.md). No window is scheduled; this is planning, not a start signal.

## Finding

FormaStack's P7 places tenant/client/user provisioning *inside* the stable publication hour W→W+60 together with GenUI's S1, observed stop, S2 and the key qualifier. GenUI cannot start browser traffic until provider identities exist: it must then build the S1 configuration, write the secret version, finish the count-zero stack change set (secret parameter, conditional execution-role policy, new task definition revision and future stop schedule), run `preflight.py --activate` and observe one healthy task behind the ALB. From the runbook that is roughly 30–40 minutes before the first browser flow. S1 (three serial browsers, native Safari included), the stopped same-ARN version transition, re-activation and S2 then need roughly 45–55 more minutes. Sequenced strictly inside W→W+60 this collides with FormaStack's P8 promotion at W+60.

## Proposal

1. **Provision before W.** Run FormaStack's provisioning ×2 and client registration (Northstar/Canary tenants, `northstar-platform`/`canary-platform` with the exact callback, Maya and the fresh invitation) immediately after P5 `/health` and JWKS are verified (about T0+1:40), not inside P7. Provisioning needs the deployed issuer, not the v2 publication.
2. **Stack update before W.** GenUI creates the campaign secret at T0 with an inert placeholder version (non-activatable by construction: the callback refuses it at startup), reserves the window and executes the count-zero change set during P1–P5. The runbook binds the secret **ARN** only, so the real S1 version is written with `PutSecretValue` once provider IDs arrive, with no task running. This is a small sequencing change to runbook step 3 (create with real JSON) and needs the activation reviewer's acknowledgement.
3. **Inside W→W+60:** W+0 `--activate` with the S1 version → healthy by about W+10 → S1 Chrome, native Safari, Firefox W+10–W+35 → key qualifier (one process, 61-second cooldown) W+35–W+40 while S1's task is still stable → observed stop, new version, `--activate` W+40–W+50 → S2 Chrome W+50–W+57 → stop and observe W+57–W+60. This fits only if W is reached as planned; if W slips past about T0+2:30, GenUI runs S1 only and records S2 unproved rather than crossing into P8.
4. **DNS.** GenUI creates the auth-staging CNAME and validation records when P5 returns them (about T0+1:15) and removes them at P12 (T0+4:30), retaining record ids privately; `beta.getforma.dev` and unrelated records are untouched.
5. **Schema 1 observation.** The read-only SQL observation task runs in P1–P2 before any GenUI activation, inside the new reservation.
6. **Cleanup.** At T0+4:30 GenUI requests count zero, observes every owned task STOPPED, reconciles durations against `extra_task_hours`, schedules `DeleteSecret` with a recovery window and reconciles the ledger; FormaStack's P12 proceeds in parallel.

No other collision was found: both sides keep APP-008 managed mutations out of the window, both treat the $5 as already reserved, and the P11 risk (v5 after cutoff) is FormaStack's recorded unproved outcome, not a GenUI dependency.
