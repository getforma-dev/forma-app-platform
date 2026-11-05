# Operations

## What this is

Beta delivery scripts. `beta/` is the operator sequence and the release record. `promotion/` is the GitHub Actions packet that promotes `main` to beta.

## Authoritative sources

- [Promote to beta](../docs/pitch/BETA-PROMOTION.md)
- [Deploy runbook](../docs/pitch/DEPLOY-RUNBOOK.md) for schema and infrastructure steps
- [ADR-017](../docs/architecture/ADR-017-BETA-PROMOTION.md) and [ADR-019](../docs/architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)

## Verify a change here

`just check py-ops py-promotion`

## Never

Do not deploy, approve a promotion, or start a second ledger. After the APP-167 cutover the local mutation recipes refuse the live private directory. Do not put secrets, account ids or approval words into Git.

## Read next

- [Infrastructure](../infra/AGENTS.md)
- [Release log](../docs/pitch/RELEASES.md)
- [Clean-trial procedure](../docs/pitch/CLEAN-TRIAL-DEPLOY.md)
- [Pitch folder](../docs/pitch/README.md)
