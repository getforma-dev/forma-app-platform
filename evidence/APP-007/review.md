# APP-007 independent local-preparation review

Reviewed revision: `8139067448a77e101a14acef1f892e8b6a943558`, including foundation commit `98df54a` and the final storage correction. This reviewer did not author or edit the application, helper, migration or infrastructure implementation.

**Verdict: local preparation is reviewable with no remaining blocking source finding. APP-007 remains Blocked and is not complete.** No GenUI deployment, RDS migration, managed readiness, two-tenant campaign or recovery trial was performed by this review. Budget/operator, actual ingress, application hostname/certificate and deployment inputs remain required before those tests. Exact-revision CI is a separate recorded requirement.

## Scope inspected

Read the Rust configuration, role adapters, native routes and readiness monitor; browser entrypoints/build; Python credential helper and qualification/cleanup commands; initial SQL migration; image and dependency locks; CDK stack/security tests; local check scripts, workflow and deployment instructions. Recomputed all **39 source-artifact hashes** from the final source-identity record against Git objects at the reviewed revision; all match. The immutable local ARM64 image remains `sha256:8d8e2f3905ec1764e14d4207915439022be6edff0791e743b502716c929f2afb`.

Native route admission and typed-props finalization occur before the listener starts. The synthetic page retains Forma’s server-rendered HTML, nonce/CSP and no-store response. The application exposes no business mutation, arbitrary role selector or generated-execution path. Fixed role adapters reload private temporary credentials. File readers reject unsafe permissions/ownership, symlinks, FIFOs and oversized input; secret debug formatting is redacted. Readiness is initially false and binds its bounded probe receipt to the current generation, original expiry and exact file bytes, rather than treating configuration presence as a successful dependency check.

The stack confines ingress to operator HTTPS, ALB-to-task and task-to-private-database. Runtime identity can assume only the six declared logical roles; fixed database usernames and separate tenant prefixes/keys are explicit. The migration secret goes only to a separate migration execution role. Image digest and architecture are required; service count starts at zero. The one-task shared trust boundary, public-IP HTTPS egress, retained resources and continuing costs are disclosed. None of those template assertions proves live tenant isolation or application authorization.

## Findings repaired before this verdict

1. The initial database probe checked ordinary role flags but could miss privileged RDS group membership. RDS’s administrator role does not require PostgreSQL `SUPERUSER`, as described by [AWS’s role documentation](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Appendix.PostgreSQL.CommonDBATasks.Roles.rds_superuser.html). The repaired probe queries the live role catalog and refuses any membership beyond the fixed login itself and `rds_iam`. The local negative control supplies `rds_superuser` membership while ordinary privilege flags are false and verifies refusal before storage access. The final image includes the repaired helper.
2. The initial template specified a 20-GiB autoscaling maximum for a 20-GiB allocation. An enabled autoscaling threshold is subject to [AWS’s minimum-growth constraints](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_PIOPS.Autoscaling.html); the intended configuration is fixed storage. The final source omits the autoscaling property, and a template assertion now requires its absence. Independent inspection confirms `AllocatedStorage: 20` and no `MaxAllocatedStorage`. No AWS deployment failure was manufactured or claimed; this was caught during source review.

Earlier source/template identities and implementation failures are retained in the evidence directory. The revised image/source identities are explicit.

## Independent checks actually run

- Ten Python failure-path tests passed, including credential rotation/expiry, special-file refusal, fixed identity, privileged membership and cleanup-scope checks. These use local fixtures and mocks; no managed service was contacted.
- Four Rust tests passed for configuration refusal, stale/generation-bound readiness and native-route admission.
- Ten CDK template tests passed, including the fixed-storage assertion. Inspected the synthesized database resource directly.
- Repeated the actual immutable image’s qualification with no network, a read-only root, UID65532, all capabilities dropped, no new privileges, two CPUs, a one-GiB limit and a private named volume. Volume mode0700 and atomic-file mode0600 passed; liveness returned200, absent managed evidence kept readiness503, HTML contained the server-rendered7, CSP nonces differed between responses, no-store remained, and unsafe startup credentials were refused without disclosure. Temporary containers and volume were removed. [Independent image record](review-image.json).
- `git diff --check` passed. The implementer’s separate full local report records all fourteen preparation steps, including locked installs, browser typecheck/build, native build and lookup-free synthesis. This reviewer inspected those results rather than relabeling them as independently repeated steps. [Review evidence](review-evidence.json).

## Required managed acceptance remains open

Before APP-007 can become Done, observe the actual migration and SQL grants, scoped database/S3/KMS connectivity and cross-tenant denials, private volume initialization on ECS, credential refresh/expiry failures, TLS/operator-only ingress, image rollout/rollback, exact-version qualification cleanup and database restore/recovery. Resolve unknown write outcomes with the retained run UUID and exact object versions before retrying. The migration and managed trial instructions are prepared, not executed. Template tests, fake unit credentials and local Docker success cannot substitute for this evidence, and they do not clear FormaStack authentication/integration gates.
