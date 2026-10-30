# APP-074 — leaked access code plus the invitee's email (runbook note for APP-076)

Date 2026-10-30 UTC. Source: the APP-074 Tier A review, minor m7 and open question 4 ([independent-review.md](independent-review.md)). This is a note for APP-076's operator runbook, not a hosted result: nothing here was run against the deployed fAuth.

## What can happen

An access code is a bearer value until it is redeemed. Someone who holds a leaked code **and** knows the address the invitation names can take the invitee's place:

1. They enter the code at `/invite` and reach fAuth's hosted registration for the invited request; fAuth accepts the invited email (merged contract §Invitation-first account creation; fAuth `hosted_code.rs:375-439` at `1bf5d3b`) and they choose their own password. The provider invitation is consumed; the code's pending admission is bound to the new account's subject (ADR-006 Flow 2).
2. fAuth mails the verification link to the **real** invitee. An invitee who is expecting an invitation is likely to click it.
3. Once the address is verified, the attacker signs in through the organization's `/signin/{handle}` link; the pending admission redeems (Flow 4) and the attacker is a member of the trial organization.
4. A later provider password reset by the invitee does not end the attacker's GenUI workspace session (up to 12 hours absolute, 1 hour idle).

Without the click the invitee is only locked out (a denial of service): their own registration answers that the account exists, and the code's one redemption is held by the attacker's pending admission until it lapses (24 hours) or is revoked.

## What the invitee is told (send with every code)

- Create your account yourself, right after you receive the code, at the link you were given.
- **Never confirm a verification email for an account you did not just create.** If a verification email arrives before you registered, or registration says the account already exists, do not click anything: tell the operator.
- Keep the code private until you have used it; it works once.

## What the operator does

Each step names the closed operator command; identifiers come from `admission list` (never the code itself).

1. **Stop the code.** `admission_operator.py revoke --organization ORG --code-id CODE`. The code refuses at once (database); its capability leaves the environment's secret (ADR-003 amendment 2026-10-30) and the provider invitation is revoked remotely. The running service drops the capability at its next task start; it cannot hand off meanwhile because the code itself refuses.
2. **If the attacker was admitted**, end their authority: `admission_operator.py revoke-entitlement --organization ORG --principal PRINCIPAL` (or `revoke-membership`). Their workspace session ends at its next request (the entitlement and membership epoch are checked on every lookup).
3. **At the provider**, revoke that account's sessions and disable or delete it (fAuth console or management API; FormaStack to confirm the exact call), so the leaked credential cannot sign in again.
4. **Reissue** to the real invitee: `admission_operator.py issue … --provider-email <invitee>` (a fresh code and provider invitation). The new capability reaches the service at its next task start: force a new deployment of the service after issuing (ECS `update-service --force-new-deployment`, or the APP-076 activation path); note that a restart also ends every in-process session and limit window (D7.11, D7.18).
5. **Record** the incident (code id, principal id, provider account id, times) in the operator log; never the code, token or password.

## Ask to FormaStack (paragraph the coordinator can send)

> GenUI's beta issues invitation-bound codes, and fAuth's hosted registration accepts the invited email from whoever holds the code. If a code leaks together with the invitee's address, an attacker can register with their own password and fAuth then mails the verification link to the real invitee; if the invitee confirms it (likely, since they expect an invitation), the attacker owns a verified account bound to that invitation. Could the verification email and its confirmation page show when and from where the account was created, with a plain "I did not create this account" action that cancels the unverified account and restores the invitation, instead of verifying it? Until then our runbook tells invitees never to confirm a verification email for an account they did not just create, and our operator revokes the code, the GenUI entitlement and the provider sessions and reissues. Please also confirm the management or console call that revokes one account's sessions, so the runbook can name it.
