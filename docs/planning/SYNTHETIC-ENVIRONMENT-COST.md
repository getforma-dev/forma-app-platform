# APP-007 synthetic environment cost estimate

Prepared 2026-10-28 for account 877573132739, us-east-1. Victor has selected **$100/month**, `beta.getforma.dev` and public HTTPS with invite-first application admission. No billable GenUI application environment has been deployed. A dedicated non-exportable ACM certificate and its DNS validation are being prepared; AWS charges no certificate fee for this integrated-service certificate type. [ACM pricing](https://aws.amazon.com/certificate-manager/pricing/).

## Current bounded Phase 1 decision

Preserve the qualified 2-vCPU/8-GiB Fargate workload, private managed RDS and tenant S3/KMS controls for a **maximum 48-hour qualification window**. This is a synthetic trial, not an always-on external-beta release. The $100 ceiling governs new project costs; no additional auth allowance or model spend is silently added on top.

- Reserve **$70** for the GenUI window and its retained infrastructure, **$5** for the separately coordinated auth rehearsal, and leave **$25 unallocated** for uncertainty. Model/provider calls remain disabled in Phase 1; later generation requires APP-043's enforceable cost admission.
- Conservatively account for a full calendar month of retained RDS, ALB, two ALB IPv4 addresses and tenant keys, even if the task stops early. The 730-hour baseline is about $51.39; a 31-day month is slightly higher. Add at most 48 task-hours, a stop margin, separately bounded migration/deployment task-hours, and at least $10 for variable usage. The actual preflight must use the current calendar month and carry earlier spent/reserved usage; every redeployment must not reset the monthly allowance.
- A one-time, service-scoped stop will set the ECS service to zero at the recorded expiry. This is an operational backstop, **not a hard AWS billing cutoff**. It does not stop independently launched tasks, remove the ALB/database, delete retained objects/backups, or cancel charges already incurred. The operator must stop owned one-off tasks, reconcile inventory and complete the recorded cleanup/retention plan.
- Before provisioning, retain the exact reviewed template/image/architecture, approved origin/certificate, expiry, current cost reservation and cleanup ownership. Record actual billed/estimated usage and retained resources before another test window. No renewal may exceed the remaining monthly allowance.
- The planned auth rehearsal is separate work owned by FormaStack; coordinate its six-hour window only when the GenUI consumer can run. Its $5 is reserved within this $100 total and is not evidence that a hosted service exists.

The steady-state hosting plan must be requalified before inviting external beta users. A fixed-CPU 2-vCPU/8-GiB VM is permitted by demo D10, but reviewed `m6g.large` core infrastructure is approximately $89.92/month before auth, model calls, requests, logs and tax. Current idle S3/KMS readiness probes can add about $3.85/month before shared free tiers. A burstable VM cannot be presented as fixed-cost sustained capacity. Neither alternative currently proves an all-in, always-on $100 beta.

## Historical always-on estimate — not the selected operating plan

The baseline is one continuously running Linux/x86 ECS Fargate task, 2 vCPU / 8 GiB, a two-zone HTTPS application load balancer, one private single-AZ PostgreSQL db.t4g.small instance with 20 GiB gp3, and two tenant KMS keys. The CDK plan uses no NAT gateway. Prices below assume 730 hours per month and on-demand rates without credits or commitments.

| Component | Rate and assumption | Approximate USD/month |
| --- | --- | ---: |
| Fargate task | 2 × $0.000011244/vCPU-second + 8 × $0.000001235/GiB-second | 85.06 |
| PostgreSQL instance | $0.032/hour | 23.36 |
| Database storage | 20 GiB × $0.115/GiB-month | 2.30 |
| Load balancer fixed charge | $0.0225/hour | 16.43 |
| Public IPv4 | Three addresses × $0.005/hour, two for ingress and one task | 10.95 |
| Tenant keys | Two new keys × $1/month | 2.00 |
| Baseline subtotal | Before variable usage | 140.10 |

Allow approximately **$150–170/month** for a lightly used, always-on synthetic environment, including a modest allowance for load-balancer usage, S3, logs, ECR, secrets and key requests. Load-balancer usage is $0.008 per LCU-hour; sustained traffic or large logs can exceed this allowance. Additional migration tasks and overlapping tasks during deployment add temporary charges. Backups beyond the included allowance, retained data, cross-zone/data transfer, support, taxes, model-provider calls and separate fAuth services are excluded. A budget is an alert/planning threshold, not a guaranteed AWS spending cutoff.

The earlier $200/month option was not approved. Victor chose $100, so the bounded operating decision above supersedes the continuous-runtime assumption. Turning off only the application task leaves database, ingress and storage charges; teardown/retention must be handled deliberately. A proposed expiry control is not counted as deployed or tested until its owning evidence proves it.

Sources checked on the preparation date: [Fargate pricing](https://aws.amazon.com/fargate/pricing/), [load-balancer pricing](https://aws.amazon.com/elasticloadbalancing/pricing/), [IPv4 pricing](https://aws.amazon.com/vpc/pricing/), and [KMS pricing](https://aws.amazon.com/kms/pricing/). RDS rates were read from the account-accessible AWS Price List Query API and retained with product identities in [cost inputs](../../evidence/APP-007/cost-inputs-2026-10-28.json). This is a planning calculation, not a quoted bill or a full managed performance qualification.
