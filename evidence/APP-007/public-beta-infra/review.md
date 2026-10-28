# Independent review: APP-007 public beta infrastructure

**Verdict: pass for the frozen local source candidate. No blocking source finding remains. APP-007 and managed acceptance remain open.**

The independent reviewer was `/root/beta_infra_independent_review`; `/root/beta_budget_architecture` authored the implementation. Review covered public/operator ingress, the bounded service window, exact stop authority, activation failure boundaries, cumulative budget accounting and operating instructions. No AWS API, provision, deployment or live identity flow ran in this review.

The candidate is bound to eleven source/config/test hashes in [review.json](review.json), verified unchanged before and after independent checks. The producer [qualification.json](qualification.json) SHA-256 was `06de066d9113d3594b0eb13083f097224ec4f9408c3590901e7732ffad4e6fc1`. Base HEAD was `d47228a3e2ea627c8d79e8298bd8d7253cf7d484`; these working-tree changes are not represented as already committed.

## Verification

| Independent command or check | Result |
| --- | --- |
| `npm run typecheck` in `infra/` | Pass |
| `npm test` in `infra/` | 12 Node tests pass: 11 template tests plus the Python wrapper |
| `python3 -m unittest discover -s test -p preflight_test.py -v` in `infra/` | 15 Python tests pass |
| `npm run synth -- --quiet` in `infra/` | Pass; account-agnostic, no lookups |
| Separate reviewer probe, retained with output and source in [review.json](review.json) | 45 checks pass; AWS-shaped calls are in-memory fakes |

The separate probe attempts account/parameter/stack drift, wrong schedule account, disabled/non-UTC/flexible/changed expiry, wrong action/role/retry/cluster/service/task, duplicate JSON, boolean/float/string/null count, too-late activation and response crossing expiry. It also checks exact time boundaries, immutable-window reuse, cumulative reservation refusal, invalid/stale accounting, and a new-year split with missing or over-budget January reconciliation. Refused activation cases make no count-one call; the late-response case requests count one then a compensating zero.

## Review repairs

1. A slow stack deployment could miss the one-time stop when count one was part of deployment. The template now initializes only count zero; the explicit activation path checks the completed live stack and a deadline at least ten minutes ahead immediately before requesting count one. Tests cover expired or too-close deadlines.
2. Reserving extra task hours only in the first month understated month-crossing windows. Every touched month now receives the full extra-task allowance, and a near-limit January fixture rejects.
3. The reviewer reproduced `DesiredCount: false` being accepted as numeric zero by Python equality, followed by a count-one activation call. The verifier now requires exact integer zero. Independent boolean/float/string/null negatives pass; the producer also checks live and response counts strictly.
4. The source/runbook incorrectly implied every CloudFormation update stopped compute. Updated instructions distinguish template defaults from actual ECS drift and require an explicit stop plus observed stopped service/tasks before changing the image, window or sole stop schedule. An ingress-only update may preserve an already active count-one service.

Earlier producer runs and development repairs remain retained; the final verdict applies only to the hashes recorded here.

## Boundaries confirmed in source

Only ALB TCP 443 switches between the operator's temporary public IPv4 /32 and `0.0.0.0/0`. Task/database inbound rules remain security-group scoped. Both tasks retain the design's 2-vCPU/8-GiB envelope, with private managed RDS, versioned encrypted S3, tenant keys and existing role boundaries.

The one-time UTC Scheduler target uses the SDK action `ecs:updateService`, exact cluster/service data and numeric zero. Its execution role grants `ecs:UpdateService` on the exact service ARN with the exact cluster condition. Trust is restricted to Scheduler, the account and dedicated **schedule group**, matching [AWS's required group-level SourceArn](https://docs.aws.amazon.com/scheduler/latest/UserGuide/cross-service-confused-deputy-prevention.html). [ECS supports this service/cluster scope](https://docs.aws.amazon.com/service-authorization/latest/reference/list_ecs.html). IAM does not constrain the desired-count field; reviewed target data supplies zero.

Budget preflight retains a full calendar month of fixed resources, all prior reservations, the active window and one-hour margin, extra task hours in every touched month, actual extra spend and at least $10 variable allowance. It enforces $70 GenUI, at least $5 auth and $25 unallocated within $100. The 31-day, 48-hour fixture totals **$68.49 GenUI / $98.49 combined**. A shorter initial window leaves more room for additional qualification. The private locked ledger persists reservations before activation and refuses missing or stale reconciliation; it is operator-maintained accounting, not a billing feed.

## Limits and required managed evidence

The supported commands and runbook are not an AWS-enforced prohibition on arbitrary privileged API calls. Initial template count zero does not prove an existing service is stopped. Before replacing the image/window/sole stop, retain actual desired/running/pending zero and STOPPED task evidence. Track and stop one-off tasks separately.

**Actual Scheduler delivery remains unproved.** AWS does not validate universal-target business input at creation, and a successfully created schedule can still fail at invocation ([AWS troubleshooting](https://docs.aws.amazon.com/scheduler/latest/UserGuide/troubleshooting.html)). The managed trial must observe a short scheduled stop, service/task outcomes and sanitized invocation evidence. The backstop is not a billing cap or teardown: RDS, ALB, IPv4, storage and keys continue charging.

Before managed acceptance, verify the real account, reviewed image/architecture, issued hostname certificate, migration, SQL role grants, paired tenant denials, credential refresh, readiness, TLS, and cleanup/rollback. Restore public HTTPS after the /32 trial. Invite-first signup, verified identity, tenant membership and app/trial entitlement remain their owning tickets' responsibility; this infrastructure change does not implement or qualify them. Model spending remains disabled. The $5 auth rehearsal must stay within the $100 total.

This review supports source landing and the next managed qualification step. It does not pass APP-007, PH01, or an external-beta launch gate.
