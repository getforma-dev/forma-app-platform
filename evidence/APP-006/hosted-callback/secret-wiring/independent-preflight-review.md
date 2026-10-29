# Independent preflight extension review

No remaining blocking finding in the final reviewed source. [The JSON receipt](independent-preflight-review.json) records final source hashes, checks and limits; [the initial failure](independent-preflight-initial-failure.json) is preserved unchanged.

The first revision checked only the live secret list. I reproduced three cases where a disabled plan reached the mocked start call despite qualification configuration supplied through a plain JSON variable, file variable or environment file. Root repaired this by rejecting every plain `GENUI_AUTH_QUALIFICATION_*` entry and every nonempty environment-file list, and also rejecting startup overrides. All **12 independent disabled/enabled bypass cases** now refuse before any mocked `UpdateService`.

The final extension retains strict same-account/region/name ARN validation, exact task revision/image/architecture and sole workspace-container checks, and exact secret injection without additional entries. Legacy deployed stacks missing the new parameter remain acceptable only when disabled. An enabled, changed or removed ARN cannot repurpose an existing reserved window.

I independently compared the new reservation against the prior committed implementation: absent and explicitly empty values retain the same previous input hash and unchanged ledger without another reservation charge. Explicit empty ECS startup/environment-file lists and ordinary runtime environment remain accepted in both disabled and correctly enabled positive fixtures.

**22 focused tests passed**, including the retained concurrency, spending and expiry cases; diff validation passed. Final hashes are `47037e5a307195efa1e95e26fc4d3925fbc8b7657d9bbfe02836444d8c5d34cd` for preflight.py and `d28105ea08bc8da886cdb84a34f3cc40ef693d7979ed1cd386f81351858bd23f` for its tests.

This review binds the ARN and task definition, not the secret value/version. Rust retains configuration validation. Final CI, actual ECS secret delivery, task health and hosted identity acceptance remain separate. Sequential control-plane observations do not make deployment atomic; unknown outcomes still require reconciliation. The reviewer made no live calls, read no credentials, and changed no implementation or staged files.
