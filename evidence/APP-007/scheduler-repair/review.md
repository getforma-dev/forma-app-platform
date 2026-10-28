# APP-007 scheduler repair — independent review

Reviewed 2026-10-28 by a separate Codex agent that did not author this scheduler change. **Accepted for the scoped managed trial; no blocking source findings.** This is not a successful redeployment or APP-007 acceptance. [The machine-readable record](review.json) binds all reviewed files and preserves exact commands and the independent comparison program.

The production change only replaces `DeploymentExpiryScheduleArn`'s schedule `GetAtt Arn` with the documented ARN assembled from partition, region, account and the actual group/schedule `Ref` values. AWS documents that both resource references return their names and that schedule ARNs include both names. The execution role correctly continues to trust the schedule-group ARN, with the account restriction intact. [Schedule Ref](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-scheduler-schedule.html#aws-resource-scheduler-schedule-return-values), [group Ref](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-scheduler-schedulegroup.html#aws-resource-scheduler-schedulegroup-return-values), [ARN contract](https://docs.aws.amazon.com/service-authorization/latest/reference/list_scheduler.html), [trust guidance](https://docs.aws.amazon.com/scheduler/latest/UserGuide/cross-service-confused-deputy-prevention.html).

I independently synthesized the baseline Git source and repaired source in memory under the same stack ID. All 64 resource objects and every other template field are deeply equal after removing only the changed output. The schedule, exact service/cluster stop target, zero desired count, account/group trust, IAM grants, retries and expiration parameters are unchanged. Source hashes match the frozen [author qualification](qualification.json).

The existing activation preflight still requires a complete stack with matching reserved parameters, calls `GetSchedule` with the exact custom group and schedule name, and compares the returned ARN and stop configuration before any count-one request. The constructed output therefore does not substitute for a live schedule check. Focused independent reruns passed:

- Two expiry template tests, including the new output-format test.
- TypeScript type checking.
- Two activation tests: the exact group/name lookup succeeds; eight altered-state cases, including a different returned ARN and a mismatched group output, all refuse before `update-service`.
- The complete before/after template comparison described above.

The [retained lookup events](../managed/initial-scheduler-lookup.json) show a successful group-qualified CloudFormation `GetSchedule` at 22:26:35 UTC, followed by a failed call without the group at 22:26:51 UTC. This supports the narrow workaround: AWS documents that an omitted group searches `default`. The provider's internal reason for omitting the group remains unknown. [GetSchedule behavior](https://docs.aws.amazon.com/scheduler/latest/APIReference/API_GetSchedule.html).

No AWS calls, credential reads, source edits, activation or cleanup occurred during this review. Root still owns failed-stack reconciliation and the next managed run, including actual ARN comparison, stopping behavior and cleanup. Retain the prior failure and any new failure; this source pass does not imply that a later CloudFormation lifecycle step or scheduled invocation will succeed.
