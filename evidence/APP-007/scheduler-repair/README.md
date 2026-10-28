# APP-007 Scheduler output repair — 2026-10-28

The first count-zero stack deployment created its expiry schedule, then rolled back because CloudFormation could not retrieve that schedule's `Arn` attribute. The repair constructs `DeploymentExpiryScheduleArn` from the documented ARN format and the actual group/schedule name references. The only generated-template change is that output value.

## Observed failure

The [sanitized event subset](observed-failure.json) retains the private source digest and selected events. `ExpiryScheduleGroup` reached `CREATE_COMPLETE` at 22:16:46 UTC, and `DeploymentExpiry` reached `CREATE_COMPLETE` at 22:26:35 UTC. At 22:26:51 UTC the stack entered `ROLLBACK_IN_PROGRESS`, reporting an unavailable schedule `Arn` attribute. Both schedule and group subsequently reached `DELETE_COMPLETE`. The retained ECS service properties specify desired count zero. No live task-state query was made by this repair author; the operator reported no app task started.

The separately retained [sanitized CloudTrail events](../managed/initial-scheduler-lookup.json) show a successful CloudFormation `GetSchedule` with the dedicated group at 22:26:35 UTC, followed by a failed request at 22:26:51 UTC with no group name and `ResourceNotFoundException`. Rollback reads supplied the group again. The missing group name in the failed request is observed, not inferred. AWS documents that omitting the group searches the default group, consistent with this dedicated-group schedule not being found. The internal provider reason for omitting it and broader provider behavior remain unestablished. [GetSchedule API](https://docs.aws.amazon.com/scheduler/latest/APIReference/API_GetSchedule.html)

## Change and controls

[The stack](../../../infra/src/stack.ts) now joins partition, region, account, `ExpiryScheduleGroup.Ref` and `DeploymentExpiry.Ref` into `arn:<partition>:scheduler:<region>:<account>:schedule/<group>/<name>`. Both resources' `Ref` values return their names, including generated names. The ARN format includes both group and schedule. [Schedule Ref](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-scheduler-schedule.html#aws-resource-scheduler-schedule-return-values), [ScheduleGroup Ref](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-scheduler-schedulegroup.html#aws-resource-scheduler-schedulegroup-return-values), [Scheduler ARN format](https://docs.aws.amazon.com/service-authorization/latest/reference/list_scheduler.html)

A before/after synthesized-template comparison confirms every resource, property, policy and other output is identical. Execution-role trust remains restricted to the exact account and schedule-group ARN, as AWS requires. The stop target remains the same service and cluster with desired count zero. [Scheduler trust guidance](https://docs.aws.amazon.com/scheduler/latest/UserGuide/cross-service-confused-deputy-prevention.html)

The existing live activation preflight still calls `GetSchedule` with the exact group/name and compares the returned ARN, state, timezone, expiry, target role, cluster/service, retry policy and desired count before requesting count one. The repaired output is an expected identity, not evidence that a schedule exists or works.

## Qualification

The [source hashes and results](qualification.json) bind the exact repair and tests. Commands ran from `infra/`, except the final diff check.

- The new focused ARN-output test failed against the original source, then passed with the repair. Two different name/account/region/partition fixtures verify the exact full ARN and ensure no schedule attribute lookup remains.
- `npm run typecheck` passed.
- `npm test` passed all **13 Node subtests**, including its wrapper of **15 Python preflight tests**. Added preflight assertions confirm the exact group/name request and reject a mismatched live ARN or group output before service activation.
- `npm run synth -- --quiet --output /tmp/genui-app007-scheduler-repair-synth` passed with `--no-lookups`, credential-file paths set to `/dev/null`, credential/profile environment variables removed, and metadata access disabled. Generated output stayed outside Git.
- A baseline/current template deep comparison found only `Outputs.DeploymentExpiryScheduleArn.Value` changed.
- `git diff --check` passed.

This is author qualification, not independent review or successful managed redeployment. No cloud mutation, credential read, activation or stop trial was performed by this author. Retry, real stop behavior and APP-007 acceptance remain separate gates.
