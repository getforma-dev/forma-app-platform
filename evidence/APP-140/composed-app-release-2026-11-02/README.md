# Composed app release and real owner verification

The composed-app implementation is deployed to **https://beta.getforma.dev**, activated at **2026-11-02 00:40:47 UTC**. Runtime candidate `9ee4a01b1c4b1b90147928844805b7397aa40903` was merged by [PR 38](https://github.com/getforma-dev/forma-app-platform/pull/38) into the active MVP trunk as `d10cf899e3261d350dc4db06dadec1e6a11a0fa0`; their source trees are identical. Annotated release tag `beta/20261102-9ee4a01` is pushed and remotely resolves to the qualified candidate. This packet records actual AWS and owner-browser observations, rather than treating local qualification as deployment evidence.

## Released behavior

APP-155–159 now supply ordered app sections, a coherent standalone/united/shared presentation, persisted finite themes, generic component dialogs and navigation, and actual record-backed lists, summaries, monthly trends, rankings and preset budgets. Owners edit these through **Edit app**. The empty Dining tracker is an ordinary template using the same general mechanism as the unrelated Equipment requests app. No model-generated executable code, tenant-specific runtime branch or canned metric supplies these features.

## Qualification and rollout

[Exact-head CI](ci.json) records all **28 successful checks**, including 24 successful push/PR workflow runs. All three ARM64 release images passed their local image qualifications; the operator's packaged receipt validator and real migration/repeat/catalog checks were exercised. Service, qualification and operator registry indexes, unique ARM64 children and configuration digests are recorded separately in the [runtime evidence](runtime.json). Docker's upload transport failed for the latter two images; independently reviewed bounded OCI transfers preserved every original byte, followed by exact immutable-tag readback. No conflicting tag was replaced or image rebuilt to bypass the failure.

The reviewed CloudFormation template remained structurally identical across 105 resources. Only three image parameters and the expiry changed, producing six task-definition replacements, two service modifications and two schedule modifications. Both services were explicitly stopped before execution and remained at zero through migration. The existing catalog 18 was verified by the candidate operator before an encrypted snapshot reached AVAILABLE for the exact source database. Five distinct, sequential operator tasks exited with numeric zero: baseline 18, migrate 19, read-only catalog 19, repeat migrate 19, read-only catalog 19. Both migration results match all 19 immutable checksums; the already-current runner returns before applying migration statements.

Actual main task definition 15 and broker definition 3 reached **1 desired / 1 running / 0 pending**, completed rollouts and healthy targets. Their running image identities match the qualified registry chains. Roles, secret references and environment boundaries were preserved. All five standard public probes passed at 00:41:13 UTC; a separate verified-TLS `/health/ready` request returned 200 at 00:42:10 UTC. Both shutdown schedules remain enabled for November 30 at 16:00 UTC. The single ledger retains all prior history and allowances, with the canonical forecast unchanged at $169.83 GenUI / $207.83 project, within the existing $170/$208 caps.

Each exact-image registry scan completed with 0 Critical, 9 High, 10 Medium and 3 Low findings. No new finding was introduced versus the previously accepted release. Two previous util-linux High entries were absent from the new scanner result; package pins were unchanged, so this is not claimed as a new package fix or a vulnerability-free image.

## Real owner journey

[Hosted owner evidence](owner-smoke.json) records a fresh ordinary sign-in through the real identity provider. At 00:42:24–00:42:26 UTC, the existing record and share inventories were exactly equal to their pre-deployment values: the original saved record and pending invitation remain. The admitted template inventory contains five entries.

At 00:42:33–00:42:52 UTC, the signed-in owner then completed these ordinary UI steps:

1. Create an empty Dining tracker through the template picker, observing real zero metrics.
2. Use **Edit app** to persist a dark/green/mono theme, a clearly labeled sample-data introduction and a generic **Log meal** button; reload successfully.
3. Open its form dialog and submit one actual **12.50** meal command. The real audit receipt, record list and dashboard agree after reload.
4. Open the dialog at 390px, then open the same themed app and dialog in the united canvas.

All four checks passed with zero browser errors. Three private screenshots were visually inspected and their hashes are retained. This used the real provider session and ordinary commands; no session helper, privileged row insertion, seed replay, new invitation, provider reconfiguration or secret rotation supplied the result. Raw user identifiers, app locators, credentials, browser profiles and screenshots remain private.

The broader [member](../../APP-160/member-browser-2026-11-01/README.md), [guest](../../APP-160/guest-browser-2026-11-01/README.md), [keyboard/canvas](../../APP-160/keyboard-canvas-2026-11-01/README.md) and [palette](../../APP-160/palette-2026-11-01/README.md) packets retain the deeper local failure/recovery, filters, exact arithmetic, live-update and revocation scenarios. Local guest actors do not establish the real recipient's signup acceptance.

## Remaining acceptance

The genuine recipient still needs to complete Forma's product-email verification before the hosted share-to-signup/acceptance rehearsal can finish. Manual screen-reader acceptance remains open. APP-161 sleep/wake remains explicitly future work. These separate obligations and older phase dependencies are not closed by this deployment; APP-155–160 ticket fields now distinguish delivered implementation from their formal acceptance gates.

After schema 19, rollback requires a compatible runtime or a separately reviewed snapshot-restoration procedure. An old catalog 18 executable must not be restarted against this catalog. No restoration was needed in this rollout.
