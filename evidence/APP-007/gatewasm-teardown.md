# GateWASM teardown — authorized 2026-10-28

This supersedes the earlier temporary [shutdown record](gatewasm-shutdown.md). Victor then explicitly authorized deletion: “there is no real data in there....so if you need to delte and tear down do it please. As we have the cdk to build it back up.” Scope is the identified GateWASM AWS application environment in account `877573132739`, us-east-1. CDK source/Git, local credentials, the shared CDK bootstrap, AWS-managed account keys and unrelated/shared infrastructure are preserved.

## Preserved rebuild record

- Owning repository `getforma-dev/gatewasm`, source revision `b750ca534ae89066714f100eb2b76ee71edf1ab1`.
- Existing TypeScript CDK files and lockfile remain unchanged in the owning repository.
- [Pre-deletion inventory](gatewasm-teardown-inventory.json) records the four exact stack IDs, resource identities/types, live deletion policies, external GateWASM candidates, image identities and CDK file hashes. No secret values, database rows or object contents were copied.
- [Action receipts](gatewasm-teardown-actions.json) record individually verified retained-resource cleanup and recoverable secret deletion dates.

AWS now closes App Runner to new customers, while existing customers may still use it and create resources; this is not a claim that the existing service had already ceased operating. A future hosting choice belongs to FormaStack's auth work. [AWS availability statement](https://docs.aws.amazon.com/apprunner/latest/dg/apprunner-availability-change.html).

## Completed teardown and verification

Final audit: **2026-10-28T19:29:59.600723+00:00**, account `877573132739`, region `us-east-1`. All 27 recorded verification commands succeeded. [Verification commands and results](gatewasm-teardown-verification.json).

- All four owner stacks — `GatewasmServiceStack`, `GatewasmCdnStack`, `GatewasmDatabaseStack`, `GatewasmNetworkStack` — are **DELETE_COMPLETE**, verified by exact stack IDs. The App Runner service first paused successfully, operation `6caa5ddafc214791b80f9bf8ad96f241`, before its owning stack removed it.
- No App Runner services/connectors, GateWASM CloudFront distribution, deployment Lambda, application alarms/topics/log groups, NAT Auto Scaling group, active EC2 instances, EBS volumes, Elastic IPs or GateWASM VPC/network interfaces remain in the audited inventory.
- RDS instance, all four manual snapshots and automated backups are absent. The live template retained the instance, so its protection was explicitly disabled and it was deleted with `--skip-final-snapshot --delete-automated-backups` under the no-real-data authorization. DeleteDBInstance CloudTrail event: `f9ef5ff1-6c61-487b-bb9e-5b8a07d7a58e`. The current retained subnet group and five obsolete RDS groups were also removed; obsolete groups had explicit `GatewasmStack` owner tags recorded before deletion. The database owner stack removed its generated credential.
- The external `gatewasm-server` image repository, 18 application/deployment log groups and retained CDN bucket are removed. The bucket's 3 versions/delete markers were removed before deleting the bucket.
- Seven external GateWASM secrets remain **scheduled for deletion**, with a 30-day recovery window ending approximately **2026-11-27 19:20 UTC**. Exact deadlines and ARNs are in the action receipts. They are inaccessible until restored, and AWS does not charge for secrets marked for deletion. Physical deletion after the recovery window is asynchronous. [AWS secret deletion behavior](https://docs.aws.amazon.com/secretsmanager/latest/userguide/manage_delete-secret.html).
- Only the shared `CDKToolkit` stack, its S3 asset bucket and its ECR asset repository remain in those inventories. Imported certificates, AWS-managed keys, unrelated/default account networking, account IAM, local credentials and source were preserved. The owning repository is clean at the recorded revision.

For network removal, only the suspended NAT ASG `Terminate` process was resumed; its other eight processes, including `Launch`, remained suspended before the owner stack deleted it. There is no stopped RDS instance left to trigger the seven-day automatic restart; no restart reminder or automation was created.

## Remaining cost and scope

No identified GateWASM application compute, database, backups, image repository, CDN bucket or retained logs remain to accrue their prior ongoing charges. Charges already incurred can still appear on the bill. Shared CDK bootstrap S3/ECR assets were deliberately retained and may continue to incur storage charges; this is **not** an assertion of a zero AWS account bill. Their contents were not purged because they are shared deployment infrastructure. No current dollar estimate or account-wide billing audit was performed.

Discovery and verification covered the known GateWASM environment in us-east-1 and its global CloudFront/S3 listings; this is not an all-region audit of unrelated account resources. No new GenUI infrastructure was created. GenUI still needs its approved separate deployment plan, spending cap and operator record.

## Rebuild limitations

CDK alone cannot recreate external Google/GitHub/SMTP provider credentials. Recover the scheduled secrets within their window or reissue those credentials; the hard-coded external secret ARNs in CDK context must be updated if replacements are created. New signing, CSRF, admin and MFA keys also need the owning auth provisioning workflow. No credentials were exported as part of this operation.

Recreate the external `gatewasm-server` ECR repository and build/push a reviewed immutable image before deploying the service stack; source uses an imported repository. Use the owning RUNBOOK and current FormaStack decisions. The retained source is not a promise that an unchanged old CDK command restores every external dependency. Database contents, backups and deleted image/object versions are not recoverable from the CDK source.

The auth deployment is intentionally being removed at Victor's request. FORMA-247/248 must plan a new approved deployment before live sign-in acceptance, not interpret missing infrastructure as a mysterious outage or silently recreate paid resources. GenUI APP-006/058 remain blocked on that upstream evidence.
