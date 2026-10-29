# Optional qualification secret wiring — author notes

The default remains disabled. `QualificationConfigSecretArn` accepts an empty value or one full Secrets Manager ARN in account `877573132739`, region `us-east-1`, with a bounded `genui-qualification-` name. The parameter contains only an identifier. The whole secret JSON is delivered as `GENUI_AUTH_QUALIFICATION_CONFIG_JSON`; the stack never contains its value and never injects the private-file variable.

When configured, a conditional inline policy allows only the application execution role to call `secretsmanager:GetSecretValue` on exactly that ARN. The application task receives the one corresponding secret environment entry. When empty, neither capability materializes. The existing migration secret/channel and every task-role permission are unchanged. No secret resource or custom-key decrypt grant is added.

The conditional policy intentionally has no unconditional task/service dependency: a false condition must not suppress normal default resources. The operating sequence remains mandatory: observe the service and its tasks stopped, finish the entire stack update, then activate through preflight. Root owns extending preflight to bind the selected ARN. Enabling the parameter also requires the separately reviewed runtime image that consumes JSON, writes the private file and removes the raw environment variable; it is not compatible evidence for the earlier foundation-only image.

All 15 template tests pass, including three new checks for the disabled mode, strict ARN boundary and exact enabled secret/role scope. Typecheck and diff checks pass. The typed CloudFormation model includes a narrow CDK `W2509` heuristic warning because the parameter name contains “Secret”; it is an ARN, so masking it with `NoEcho` would prevent direct live parameter comparison. This warning and exact source digests are retained in `infra-author.json`.

This is local infrastructure preparation. No AWS action, credential read, secret creation, staging or commit occurred. Root independent review, final CI and actual ECS/private-file/hosted trials remain required.
