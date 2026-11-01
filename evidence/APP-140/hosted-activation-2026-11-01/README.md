# Actual clean beta activation — November 1

The clean beta release activated at **2026-11-01 20:13:41 UTC**, observed healthy at 20:15:07 UTC. [deployment.json](deployment.json) retains only non-secret facts from reviewed window `1608eea9`: application task definition **13**, broker task definition **1**, both with one running/healthy task and completed rollout; database catalog **18**; exact qualified service/operator images; public health **200**. The [release history](../../../docs/pitch/RELEASES.md) contains the deployment tool's exact generated row.

## Source, image and tag identities

- Tested/deployed image candidate: `711d87d985cd0484bd25d2f863f6428d370881ea`.
- PR 31 merge: `6c308f1a9c7db08d019332bb14b9b235db650362`; the candidate is an ancestor. The deployed image is not relabeled with the merge SHA.
- Pushed annotated release tag: `beta/20261101-711d87d`, object `e40306141461a5d3610e1d19ca9a2e9330cbdeea`, peeling to the exact candidate `711d87d`.
- [Candidate qualification](../mvp-candidate-711d87d/README.md) retains all 28 successful checks, the full local check, image qualification, package readback and actual scans. Those exact image index identities match the running tasks; ARM64 child digests remain separately recorded.

The managed baseline catalog was verified at 13. A pre-migration snapshot became available, migration advanced the catalog to 18, and read-only catalog verification passed at 18. The repeated migration/catalog observation also completed successfully. Separate operator tasks provisioned the clean organization, verified its audit state, issued the intended owner invitation and listed it; each retained stopped-task result has operator exit code zero. Clean audit verification reported zero command outcomes and next commit sequence one. Provider readback after that invitation, before product registration, still showed zero users. These facts do not stand in for the owner's actual redemption/sign-in or the guest's later signup.

## Preserved verifier finding and corrected observation

The original [verification receipt](original-verification.json) reports `live=true`, `canvas=false`, `mcp=true`, `slack=true`. It is retained as observed. Its canvas oracle expected an anonymous sign-in page, while the application deliberately returns uniform 404 for protected `/canvas` access without a session. [Anonymous denial probes](uniform-anonymous-denial-probes.json) confirm `/canvas` and `/v1/mcp` both return identical `application/problem+json` 404 bodies. [Public entry probes](auth-entry-public-probes.json) separately confirm 200 HTML for sign-in, the privately configured owner entry and invitation entry, without creating accounts or calling the provider.

An additional activation receipt normalization corrected the local wrapper shape after service counts were reconciled. Its [receipt](activation-receipt-normalization.json) records the original and normalized hashes and **no remote mutation**. The original request/verification artifacts remain unchanged. These two operator-tooling findings are tracked in [PR 33](https://github.com/getforma-dev/forma-app-platform/pull/33), pending CI at this record. The [corrected read-only verifier](corrected-verification.json) from operator source `64ce1f0` subsequently passed all five probes: live, canvas, positive entry, MCP and Slack. This preserves the original failure and required protected-route behavior; no runtime version changed. The separate execution/recovery fix in [PR 32](https://github.com/getforma-dev/forma-app-platform/pull/32) is merged at `105bb50ee`.

## Acceptance still separate

This is actual deployment evidence, not a completed paid-user demonstration. At handoff no product registration, owner session, app or guest acceptance was claimed by these probes. The later [genuine owner journey](../../APP-139/hosted-owner-2026-11-01/README.md) separately passed actual registration/email verification/sign-in, empty-workspace app creation, a committed record, a pending private share and contextual Ask. Guest delivery still depends on the intended recipient's pending SES sandbox verification and then the separate provider verification/sign-in flow. No synthetic identity listener, cookie injection, invitation resend or provider switch substitutes for those actions.

All private addresses, sign-in handles, account IDs, ARNs, credentials, admission codes and operator request bodies are omitted. [sources.json](sources.json) records hashes for the original private receipts; the deployment and extracts can be reconciled by the authorized operator without publishing those files.
