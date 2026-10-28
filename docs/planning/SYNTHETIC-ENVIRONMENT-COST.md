# APP-007 synthetic environment cost estimate

Prepared 2026-10-28 for account 877573132739, us-east-1. No GenUI resource has been deployed. Victor is the proposed resource owner; the monthly budget is awaiting his answer. This estimate concerns the GenUI stack only, not the account's existing resources or the separate FormaStack fAuth deployment.

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

The $200/month option offered to Victor is a proposed planning threshold with room for trials, not an approved amount. A $100 threshold requires revising operating hours/resources and rechecking the design envelope. Turning off only the application task leaves database, ingress and storage charges; teardown/retention must be handled deliberately. No idle-stop behavior or automatic cleanup schedule is claimed implemented here.

Sources checked on the preparation date: [Fargate pricing](https://aws.amazon.com/fargate/pricing/), [load-balancer pricing](https://aws.amazon.com/elasticloadbalancing/pricing/), [IPv4 pricing](https://aws.amazon.com/vpc/pricing/), and [KMS pricing](https://aws.amazon.com/kms/pricing/). RDS rates were read from the account-accessible AWS Price List Query API and retained with product identities in [cost inputs](../../evidence/APP-007/cost-inputs-2026-10-28.json). This is a planning calculation, not a quoted bill or a full managed performance qualification.
