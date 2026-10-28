# Reviewing an application ticket

Independently assess one ticket against its cited design and the concrete change. Read the [board guide](../README.md), [contribution guide](../../CONTRIBUTING.md), ticket, relevant design sections and diff. Use a reviewer who did not implement the change. Review the actual revision; earlier review or CI on different source is not automatically transferable.

## What to examine

- **Outcome and scope:** Does the change deliver the ticket's behavior and preserve its authority boundaries? Is a proposed capability clearly distinguished from implemented behavior?
- **Correctness:** Examine the changed code and the relevant callers. Check empty/error states, lifecycle cleanup, concurrency, persistence and recovery where they apply.
- **Acceptance evidence:** Map each completed checklist item to a test, inspection or retained report. Confirm important negative controls can detect the claimed failure. Run focused checks when evidence is missing or a finding needs reproduction; do not rerun broad suites without a reason.
- **Security and data:** Check applicable organization/source access, stale authorization, input admission, CSP/escaping, secrets and data exposure. Focus on the surfaces this ticket changes and the design's associated requirements.
- **Forma contracts:** Confirm the chosen native/Browser mode, correct reactive accessors and island registration, typed server response policy, and actual adoption behavior when relevant. Generated files must not conceal source admission failures.
- **Integration and maintenance:** Inspect migrations/rollback, dependency pins, documentation, relevant CI on the reviewed source and board consistency. A framework release does not prove application behavior. Confirm that FormaStack-owned blockers have the required upstream evidence before GenUI claims dependent integration complete; unrelated work may proceed in parallel.

## Review result

Record the reviewed commit or exact diff identity, inspected areas, checks actually run, findings and limits. Rank actionable findings by their effect and cite precise locations. Distinguish blockers from optional suggestions. State either **changes requested** with the unresolved acceptance/correctness issues, or **no blocking findings** with the evidence supporting that conclusion.

Return the review to the implementer/lead and retain it with the ticket's evidence. Use the review surface authorized by the current task; this guide does not require sending an external message or impersonating a separate GitHub account. Do not attribute work to a model or tool that did not perform it.

After fixes, recheck the changed surface and any affected acceptance cases. Bind the final verdict to the final revision and record CI's actual state. The ticket reaches Done only through the board's acceptance and landing rules; the reviewer does not change status merely because no issue was found in a limited inspection.
