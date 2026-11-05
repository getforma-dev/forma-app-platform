# Pitch folder

The name is the pitch. The folder is not only a pitch archive. It holds live delivery procedures, the release log, product rules, historical scripts, and fixtures that code opens by these paths. A rename is deferred. Tests and `ops/` read `RELEASES.md`, `receipt-fixture.png` and `folder-fixture/`.

## Live procedures

- [BETA-PROMOTION.md](BETA-PROMOTION.md) is the one-approval path from `main` to beta.
- [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md) is the operator procedure for schema and infrastructure. After the APP-167 cutover its laptop mutation recipes refuse the live private directory.
- [CLEAN-TRIAL-DEPLOY.md](CLEAN-TRIAL-DEPLOY.md) is the clean-profile procedure on the same stack. Its November 2 status block is behind the sealed release in [RELEASES.md](RELEASES.md).
- [RELEASES.md](RELEASES.md) is the beta release log. `ops/beta` appends a row and tests read the file.
- [SLACK-SETUP.md](SLACK-SETUP.md) is the Slack secret procedure. [slack-app-manifest.json](slack-app-manifest.json) is the manifest those steps paste. The checkout path in the steps is the old clone name.

## Rules and explanation still in force

- [CANVAS-RULES.md](CANVAS-RULES.md) is the canvas grammar. [ADR-018](../architecture/ADR-018-WHOLE-APP-VIEWS.md) and [CANVAS-WINDOWS.md](../architecture/CANVAS-WINDOWS.md) override the older component-placement rules.
- [CONNECTIONS.md](CONNECTIONS.md) says how a person, an agent and the model each connect.

## Historical

[GUIDE.md](GUIDE.md), [PITCH-SCRIPT.md](PITCH-SCRIPT.md), [RECORDING.md](RECORDING.md), [REHEARSAL-2026-10-31.md](REHEARSAL-2026-10-31.md) and [SCENARIOS.md](SCENARIOS.md) are the pitch tour, the spoken script, the seeded shot list, the rehearsal log and the two-window choreography. The current trial is [first-run](../planning/first-run/README.md).

## Fixtures

`receipt-fixture.png` and `folder-fixture/` are synthetic inputs. Rust tests, the dev seed and the box campaign open them at these paths. They are not documentation.
