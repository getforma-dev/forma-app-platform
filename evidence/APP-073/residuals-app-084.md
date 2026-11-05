# APP-073 review residuals closed by APP-084

Date 2026-11-05. Each row names the review file and the finding. A limit stays a limit. A fix names the change.

| Finding | Disposition |
| --- | --- |
| Provider review M2, ADR-003 amendment 2026-10-29 | Superseded on 2026-10-30. Capabilities are in Secrets Manager under the environment key. APP-074 implemented that move. No real-customer invitation is issued under the 0600-file default. This change does not call AWS. Live secret rotation and the stack-parameter clear stay a Victor-approved mutation and were not run. |
| Provider review m8 and the audit recommendation | Fixed. ADR-007 D7.8 names `provider.invitation.created`, `provider.invitation.reconciled` and `provider.invitation.revoked`. Migration 0021 widens `operator_chain`. Each committed state change writes one event. `verify` recomputes the chain. |
| Provider review n1, n2, n5, n6, n7, n8 | The stand-in, host allowlist, `not_pending` detail, one-tenant rule, create lock-order comment and check 24 stay as they landed with APP-073. n7's lock-order comment is in `create_remote`. No new defect turned up in the provider tests. |
| Provider review m4, reconcile-only, 300 s skew | Accepted limits. Provider expiry and code expiry can diverge. `reconcile` runs only when the operator runs it. The clock-skew window is 300 seconds. |
| Admission review m4 | Landed numbering agrees: 0004 entitlement reads, 0005 definitions, 0006 sessions, 0007 commands. The operator text no longer says migration 0006 for the command sequence. |
| Admission review m5 | Recorded in ADR-007 D7.19: Phase 1 trial extension is code extension plus reissue. There is no `entitlement.extended`. |
| Admission review m6 | Recorded on plan Task 5. The seed does not insert the two digest-only codes. The campaigns issue codes through the operator command. |
| Admission review nits and implementation notes | D7.3's 0004 `entitlements` read is the landed grant. `gssencmode="disable"` and the Unix-socket refusal are in `connect`. `--provider-email` is not read from argv when the managed secret supplies it. `backfill` writes no audit event; the genesis heads are the record. A never-sent `create` that is still `pending`, plus its `revoke`, means do not create (`nothing_to_revoke`). |
| Schema-0003 review m7 | Accepted limit, record only. The runner review's m3 runtime half stays recorded. No code change. |
| Schema-0003 observations | Accepted limits. Several live pending admissions per identity are bounded by m3(a). The context digest rests on the flow record. `replayed` returns the epoch only. A compromised workspace connection is inherent to ADR-004. |
| APP-012 review m9 and B1 review m3 | Fixed for `register-source-revision`, `create-bound-work`, `bind-provider-tenant` and `seed-spend-review`. The kinds are `source_revision.registered`, `work.seeded`, `provider_tenant.bound` and `app.seeded`. An unchanged rerun writes nothing. |
| B1 review m6 / operator-gaps M4 | Fixed. `genui_private.provider_tenant_index` has no RLS and one row per issuer and tenant. Migration 0021 copies every active binding, including an organization with neither a handle nor a code, by disabling that table's RLS for the copy and turning FORCE RLS back on before commit. Bind, provision, the first handle and code issue claim or refuse against that index. |
| Operator-gaps M3 and the live secret cleanup | Not run. Deleting or rotating the live secret is an AWS mutation and needs Victor's approval. |
| APP-013 re-check N1 | Fixed in ADR-002. The idempotency scope is organization + principal + app + work + command + key. Migration 0007 already keys the claim and the outcome on those six columns. |

Prepared by Grok.
