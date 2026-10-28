# Demo scenarios and acceptance evidence

These fixtures make the proposed behavior checkable. They are synthetic, not customer evidence. Acceptance applies to a running implementation; this preparation packet alone does not pass these tests. The [shared contract](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/SHARED-CONTRACTS.md) defines authority and lifecycle semantics.

## S01 Platform home and meaningful creation

Use two organizations: Northstar and an isolation canary. Create organization-scoped Security and Finance spaces for Northstar. Verify click and slash navigation resolve to the same IDs, aliases cannot collide within the organization, and inaccessible apps do not appear in suggestions, recents or thumbnails. Unknown input and ordinary navigation produce no mutation or model request.

Create the lead app from supported library primitives. Record which components were retrieved, adapted or generated. The app's six views share selected asset/window/filter meaning, and the evidence table can explain aggregates. A supported change must alter a calculation or persisted input behavior; a new theme alone does not pass.

Exercise the actual spatial workspace: pan/zoom, move/resize, group related cards, add a card without covering current work, and recover orientation through fit/reset. Reopen the saved personal layout, then explicitly save/share a view and verify the audience difference. Complete the equivalent task through keyboard navigation and structured/focused views. A label edit must preserve unrelated filters and notes; connections indicate actual data/evidence relationships, not a claimed causal link.

Import once by dropping a supported file onto the canvas and once through the equivalent keyboard-accessible Upload control. Both produce a source card and inspection state. Check visible limits before upload, progress/cancel, worksheet selection and actionable failure. Neither path bypasses mapping confirmation or server admission. New users receive a useful starting arrangement; an empty canvas remains an explicit advanced choice.

## S02 Northstar arithmetic and source interpretation

Select asset `build-worker-17` and the window **[2026-10-22 10:00:00, 10:10:00) UTC**. The lower boundary is included and the upper boundary excluded. Each row reports a byte count attributed to that event timestamp, not a cumulative counter or a continuously sampled rate.

| Event | Time UTC | Destination | Bytes |
|---|---|---|---:|
| E1 | 10:00:00 | 203.0.113.10 | 60,000 |
| E2 | 10:01:00 | 203.0.113.20 | 120,000 |
| E3 | 10:02:00 | 198.51.100.7 | 300,000 |
| E4 | 10:03:00 | 198.51.100.7 | 600,000 |
| E5 | 10:05:00 | 203.0.113.10 | 60,000 |
| E6 | 10:10:00 | 198.51.100.99 | 900,000 |

The destination inventory has an exact entry for `203.0.113.10` and a network entry for `203.0.113.0/24`. Exact match takes precedence, otherwise use the most specific supported network match. A connection is counted once even if several inventory entries match. The absence of a match means **not found in this supplied inventory**, not malicious.

Expected results for the selected window:

- Five included events; E6 is excluded.
- Total 1,140,000 bytes: 240,000 matched and 900,000 unmatched.
- Three distinct destinations: 203.0.113.10 has 120,000 bytes, 203.0.113.20 has 120,000, and 198.51.100.7 has 900,000.
- Whole-window average 114,000 bytes/minute; unmatched average 90,000 bytes/minute. The denominator is the full ten minutes, not the five minutes between first and last included events.
- A one-minute timeline has 60,000; 120,000; 300,000; 600,000; 0; 60,000; 0; 0; 0; 0 bytes. Empty buckets are zero only after confirming complete coverage for this fixture.

Keep integer bytes exact. Do not silently reinterpret bytes as bits or decimal as binary units. Reject invalid/ambiguous timestamps and undeclared units at mapping. If a later source has cumulative counters, partial capture or a different timestamp meaning, require a supported interpretation before reusing these calculations.

Select 198.51.100.7: all linked views agree on E3/E4 and 900,000 bytes. Clear the filter: totals return to 1,140,000. A paginated table or top-destination view must not change the full-filter total. A late response from a prior filter epoch is discarded. Record input, mapping, operator and app versions in receipts.

## S03 Transform, preview and publication

Add the transfer-rate measure and an observation form to a draft with stable field IDs, evidence references, text and a bounded review status. Inspect the semantic difference and then undo it through a new revision. A stale patch against an older draft conflicts rather than replacing newer work.

Submit a trial observation in preview. It must remain in the isolated preview store. Publish the reviewed draft and start live work: no preview record is present. A reviewer must approve the exact final definition; modifying the denominator, source declaration or form schema invalidates that approval. Author self-publication fails under Northstar's independent-review policy. Separately verify authorized owner publication under a low-risk policy configured by an administrator, not chosen by the model.

A new failing draft must not disable the existing permitted published version. A model outage must leave its deterministic queries and internal forms usable. A schema-breaking draft cannot silently reinterpret existing saved work.

## S04 Same work, new work and version adoption

Save Maya's case with the selected window, source revisions and an observation: “900,000 bytes went to a destination absent from the supplied inventory; ownership and expected activity remain unverified.” Preserve its evidence links and uncertainty without upgrading it to a compromise verdict.

Share this saved work with Jordan after granting the necessary work and source access. Reopening shows the same work ID, committed records and pinned app version. A copied link without those grants reveals neither findings nor private metadata. Revocation denies subsequent raw, cached and derived responses, including a delayed result authorized before revocation.

Before revocation, exercise explicit export with an authorized principal: produce a labeled snapshot of the selected permitted findings and evidence, including its source/app revisions and capture time. Export does not grant the recipient live access or silently include unrelated private notes. Deny the same operation after revocation; disclose that an already downloaded snapshot cannot be recalled.

Start **new work** for Jordan's `ops-runner-04` using two events in the same ten-minute window: 180,000 bytes to 203.0.113.10 and 30,000 bytes to 198.51.100.8. Expected total is 210,000 bytes, unmatched total 30,000 and full-window average 21,000 bytes/minute. It has a different work ID, independent bindings and an initially empty findings collection. Maya's 1,140,000-byte case remains unchanged.

Publish a compatible new app revision. Existing cases stay pinned until an authorized user explicitly adopts it through a checked update. Show the changed meaning and record adoption. Reject incompatible record schema adoption without an approved migration. App rollback must not undo committed findings or recreate revoked access.

## S05 A second app with writable records

Use a spending table with stable expense IDs and an explicitly single currency. `amount_cents` is an integer, currency is USD; values are not floating-point dollars.

| Expense ID | Description | amount_cents | Currency |
|---|---|---:|---|
| EXP-001 | Equipment supplies | 12,000 | USD |
| EXP-002 | Hosted service | 45,000 | USD |
| EXP-003 | Team transport | 7,000 | USD |

Total is **64,000 cents ($640.00)**. A threshold of `amount_cents >= 20,000` selects only EXP-002, totaling 45,000 cents ($450.00). The UI can display all three records or the one-item queue without confusing the two scopes. Mixed-currency aggregation is rejected unless a separately supported conversion contract is supplied.

Through the same creation path used for Northstar, form a queue, detail panel and internal decision form. Use a record identity unique to expense ID within saved work. Declared fields include `status` (`needs_context`, `reviewed`) and `note`; an absent decision displays “Pending” without pretending a committed record exists. The server enforces source-reference scope, field rules and permitted transitions.

Submit EXP-002 with `status=needs_context`, note “Confirm the billing period,” expected version 0 (record absent), and key K1. The committed record is version 1. Capture its command outcome and audit. Retry the identical request with K1 after a simulated lost response: retrieve the same outcome, one record revision and one mutation audit event. Reusing K1 with a different payload conflicts. A distinct stale request expecting version 0 conflicts against version 1. After refresh, submit `reviewed` with expected version 1 and new key K2: version 2 appears.

Current authorization precedes replay lookup; an authorized replay returns its stored result before applying the now-stale expected-version check. Only a new command proceeds to compare-and-set. The record, idempotency result and mutation audit commit atomically; attempt logging must not be mistaken for a second committed mutation.

The imported spending revision and its 64,000-cent total remain unchanged after both decisions. No payment, email or source-system update occurs. Query receipts pin the internal record snapshot sequence, so a saved view can explain whether it observed version 1 or 2. Retrying after permission revocation does not reveal the earlier result to a now-unauthorized principal.

Navigate back to Security: the prior case, permitted context and personal view state return. Late Finance responses cannot populate Security cards. A security-only principal cannot read the Finance app or its decisions merely because both spaces share an organization.

## S06 Failure and adversarial cases

| Stimulus | Required observation |
|---|---|
| Log field says to export secrets or run a command | Treat as evidence; unsupported tool/operation denied by server enforcement; no external effect |
| Unapproved executable markup or unknown operation in a proposed definition | Reject admission; no execution or silent renderer fallback |
| Oversize CSV, compressed XLSX expansion, invalid sheet dimensions | Bound processing and explain rejection; no unrestricted parser work |
| Concurrent record edit or work-version adoption | Visible conflict with authorized recovery; no silent overwrite |
| Revocation or app/work version adoption races a paused new record command | Release the command after the authoritative revocation/adoption commits: stale policy/work/schema epoch cannot commit; reverse the ordering to prove the earlier authorized commit remains recorded |
| Command timeout after commit | Recover original outcome with the same key; no duplicate intent |
| Context switch during a delayed response | Drop mismatched context/filter response |
| Missing, changed or deleted source | Explain state with historical identity; do not silently substitute a newer file |
| Cross-organization ID or guessed source/work link | Deny before data or private metadata release |
| Revocation during queued query or export | Recheck at execution/delivery; deny subsequent release |
| Real-data model disclosure not configured | Explanation remains disabled; deterministic app operation continues |
| Process restart after a committed form submission | Reopen the same record/version and authorized command outcome; no mutation lost or duplicated |
| Restore from backup into an isolated environment | Recover app/work/source/record relationships and audit to the declared recovery point; measure recovery time, rather than substituting fixture reseeding |
| Authorized deletion and retention expiry | Verify original/normalized evidence, derived findings, previews, caches and backups follow the declared policy; retained tombstones contain no removed sensitive payload |
| One organization exhausts its job or model budget | Visible admission/budget response and continued service for other permitted work; no unpublished or partial generated definition becomes active |

A prompt-injection demonstration proves only the tested operation boundary. A generic model refusal is not evidence that the authorization boundary held. Capture the denied server operation and resulting absence of effect.

## S07 Evidence packet for a working demo

Retain the build/dependency pins, two admitted app definitions and digests, input fixtures, expected arithmetic, query receipts, record outcomes and a compact rehearsal log. Demonstrate the second app from the normal authoring interface on the same platform build, with no source edit or per-example redeployment. Verify both permitted colleague use and a denied canary request.

Record import-to-published-app time including mapping and review, independent reuse completion, reopen success, failure recovery and meaningful adaptation. Report sample size and conditions. Simulation, seeded state and manual intervention are disclosed. Passing a rehearsed synthetic demo does not establish customer adoption, production SLOs or suitability for regulated work.
