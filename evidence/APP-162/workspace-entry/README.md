# Workspace sign-in: local backend qualification

The new workspace-entry form reaches the existing tenant-bound authentication flow through `POST /signin`. It accepts a workspace alias, opaque handle, relative `/signin/{selector}` link or exact-origin absolute link. The ordinary handle route remains available. A submitted workspace overrides a remembered hint; **Switch workspace** opens an empty entry without changing the current account or session.

[Qualification data](qualification.json) records exact source-file hashes, actual commands and positive-selected check results. The implementation was uncommitted when these checks ran; the recorded base commit is not represented as the tested candidate. The integrating owner records the final candidate, full repository checks, browser review and release evidence separately.

## Results

| Verification | Result |
| --- | --- |
| New selector and anonymous form-security tests | 2 passed |
| Existing anonymous route/refusal test | 1 passed |
| Native SSR sign-in page test, including final button/link copy | 1 passed |
| Real PostgreSQL session campaign with an explicitly configured synthetic alias | 21 checks passed |
| Invitation regression campaign | 13 checks passed |
| Shared-identity regression campaign | 8 checks passed |

The session campaign completes actual local router → synthetic provider PAR/PKCE → callback → authorized workspace journeys for four inputs: opaque handle, relative link, full local-origin link and configured alias. Each starts a distinct flow and overrides a stale organization hint. Unknown, malformed and stored-but-unserved selectors create neither a flow nor a provider request. Existing cross-tenant, replay, token verification, expiry, revocation and admission checks still pass.

Focused tests cover duplicate/extra fields, body bounds, cross-site rejection, shared rate limits, hostile origins, userinfo, alternate ports, dot segments, percent/double encoding, fragments and query strings. A supplied URL is only a local routing selector; it is never fetched or used as a redirect destination. Native SSR remains usable without JavaScript, including a blank entry and a retry after neutral failure.

The invitation and shared-identity campaigns retain exact destinations and tenant checks. A member's workspace session remains separate from an account-only shared-app session, and rejected callbacks do not silently switch a user into another organization.

## Scope

These runs use an isolated **real PostgreSQL database with verified TLS**, the actual application router and a **synthetic local identity provider and management transport**. They are not AWS, live-mailbox or hosted-account acceptance. All test identities and the configured alias are synthetic. No trial reset, migration, production provider change or new grant supplied these results.

This change does not implement email-based workspace discovery or one global credential across tenants. Readable aliases can reveal workspace existence when a valid alias redirects to the identity provider; equal initial pages do not remove that tradeoff. Private organizations can continue using opaque links. The existing trial identity and its data need no replacement for this entry flow.

## Independent alias-operations review

The window-local alias change was independently reviewed. One finding was repaired: the first validator accepted noncanonical base32 padding bits that the application rejects. Exact decode/re-encode equality and a regression case now cover that boundary. All six focused wrapper tests passed, along with shell syntax and whitespace checks. The tests use temporary Git/CDK fixtures and an AWS shim that refuses every call. Source hashes and the review result are included in the qualification data.

The helper preserves an explicitly supplied alias across successor windows and uses the same retained context for synth and diff. It validates shape and continuity; a real deployment still needs the existing trial's provision receipt to establish that the selected handle belongs to that organization.
