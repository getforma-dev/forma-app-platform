# Infrastructure

## What this is

The AWS CDK stack for the one beta environment, `GenuiSyntheticFoundation`, in account `877573132739`, region `us-east-1`. Local synth and tests create no resources.

## Authoritative sources

- [infra/README.md](README.md)
- [ADR-013](../docs/architecture/ADR-013-REPOSITORY-AND-DELIVERY.md) and [ADR-017](../docs/architecture/ADR-017-BETA-PROMOTION.md)
- [Promote to beta](../docs/pitch/BETA-PROMOTION.md)

## Verify a change here

`just check infra py-infra`

## Never

Do not deploy from a laptop. A merge to `main` prepares a packet; AWS changes only after the protected approval. Do not change the database engine, a secret or the stack policy inside an image-only promotion.

## Read next

- [Promotion](../docs/pitch/BETA-PROMOTION.md)
- [Deploy runbook](../docs/pitch/DEPLOY-RUNBOOK.md)
- [Operations](../ops/AGENTS.md)
- [ADR-019](../docs/architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)
