# Shared platform and demo contract proposal

Contract edition: **prep-2026-10-22.1**. These recommendations bind the two prospective designs together if adopted. They are not shipped APIs or founder-approved architecture decisions. The demo records this file's digest in `platform-contract-lock.json`; a changed contract requires explicit reconciliation in both tasks.

## C01 Generic ownership and identity

Organization is the tenant boundary. A space organizes apps; an app has one owning space, one stable identity and an owner. A saved-work instance belongs to one organization and app and pins a published revision. Domain names such as case, review and board are app metadata, not different authorization engines.

Use tenant-bearing keys and relationships for every app, work item, source, internal record, publication, job, cache, index and audit event. Actor/tenant authority comes from authenticated context plus verified membership, never a model-supplied field. Schema references and foreign keys cannot silently cross that scope.

## C02 App definition and stored work

| Object | Required identity and invariant |
|---|---|
| AppDefinitionRevision | Content digest, schema version, renderer/operator versions, inputs, views/queries, forms/record schemas, commands, links, default layout and descriptive metadata; immutable |
| Publication | Exact app digest and input-schema contracts, admission result, policy version, publishing/reviewing principals, approved capabilities and time; append-only decisions/revocations; actual evidence revisions belong to each work binding |
| SavedWork | App/version, context, authorized input bindings, record collections, shared decisions/notes and version; separate from definition |
| PersonalViewState | Principal plus organization/app/work, layout, exploratory filters and selections; not automatically shared |
| SourceRevision | Immutable content or reproducible snapshot, source/mapping/parser versions, units/time meaning, row counts and provenance |
| RecordSchema | Versioned field IDs/types/constraints, permitted command/transition declarations, owner scope and source-reference rules |
| InternalRecordRevision | Stable record ID, schema/work/app scope, version, committed values, creator/updater and source dependencies; superseded values retain authorized history under retention policy |
| Binding | Exact compatible source/view or internal collection revision policy; never a reusable credential |
| QueryReceipt | Mode, app/draft digest, operator versions, sources and record snapshot sequence, parameters, actor/policy epoch, exact result digest, partial/truncation state and reproducible provenance |

Publish a reusable method, not the author's work instance. Review metadata/defaults for accidental private data. Reuse creates a fresh binding, record collection and personal state. Continue-work intentionally opens the same authorized instance; these are different UI actions.

## C03 Spaces, navigation and discovery

Click and slash resolve to the same stable organization/space/app IDs. A command alias is a unique organization-scoped convenience, not authority or an action invocation. Navigation requires Enter/selection, uses no LLM, does not create resources and never commits a form. Resolve old links through stable IDs; alias changes and app moves retain explicit history and policy checks.

Filter search, recents, thumbnails and suggestions before revealing private metadata. Semantic ranking occurs within authorized candidates, followed by fresh publication/input compatibility checks. A stale index hit cannot grant access. Switching context cancels or discards late results for the previous organization/work; unsaved work is saved or clearly retained for user recovery before leaving.

The workspace supports pan/zoom, move/resize, grouping, automatic non-obscuring placement, fit/reset and saved layouts. Keyboard navigation and a structured list/focused view operate on the same work. Connections express actual source/view/evidence relationships, not inferred causality. Save state makes personal layout/filter changes distinct from deliberately shared views and committed work; an unrelated label/layout patch preserves filters, notes and record state.

## C04 Supported app language

Recommend a closed typed AppDefinition interpreted by deployed trusted Forma renderers. It composes summary/table/detail/timeline/chart/filter/form/note/status components and declared links. Internal record schemas use stable field IDs and a bounded type set: text, boolean, integer/decimal-with-unit, date/time, enum and scoped references. No generated JavaScript, TSX, arbitrary SQL/DDL, executable HTML/CSS, URLs, paths or permission code.

Query grammar: projection, typed filters, bounded group-by, stable sort/limit, count/sum/min/max, explicit time buckets, safe aggregate arithmetic and registered lookup/comparison operators. Totals use the full authorized filtered input before result pagination; stable tie-breaks and omitted-group labels make views repeatable. Units, null handling, currencies, timezone and lookup cardinality are declared, not guessed by the renderer.

Command grammar initially permits bounded create/update of app-owned records and declared status/ownership transitions, triggered by explicit user submit. No arbitrary multi-system action graph, scheduled side effect or autonomous bulk mutation. Model output can propose schemas, queries and commands only within those capabilities; unknown or excessive structure is rejected. Requests outside the grammar abstain and explain the missing capability.

## C05 Source import and interpretation

CSV and selected tabular XLSX imports begin through equivalent visible drop-zone and keyboard-accessible Upload controls. Dropping a supported file creates a source card and inspection state, not an immediately trusted usable source. Show format/limit restrictions before upload, progress/cancellation, worksheet/table selection, accepted/rejected counts, mappings and transformations. Never execute macros, external links or a general spreadsheet engine. Literal stored values are supported; formula-cache use needs an explicit visible policy and provenance, otherwise the mapping is rejected. Bound compressed bytes, expanded bytes, entry/cell counts, dimensions, time and memory before materialization.

Preserve original values, stable logical CSV-record identity/physical span where possible, and XLSX sheet/row/cell coordinates. Replacements are new revisions. Confirm units, timezone, source timestamp/interval semantics, matching keys and source-specific interpretation. Missing values are unknown; material ambiguity is not silently repaired. Large integers avoid browser precision loss. Currency totals cannot cross currencies without an explicit supported conversion contract.

Sources can be absent for an app that collects only internal records. Source access does not authorize app publication, record export or model disclosure. Retention and deletion apply to original/normalized data and derived content; a deleted source yields a historical tombstone, not silent substitution with a newer file.

## C06 Query execution and provenance

Authenticate, authorize discovery/run/read scope, resolve exact app and bindings, validate typed operations/budgets, execute, and recheck authority before release. Queued work checks at execution and result delivery. Receipts pin immutable input and query versions; mutable records use a consistent committed snapshot/sequence retained long enough to explain saved decisions.

Aggregate provenance stores the reproducible predicate and revision references plus bounded drilldown, not a million row IDs in a browser payload. Cards show loading/empty/error/denied/stale/partial distinctly. Linked results are associated with the same filter epoch so a late old response cannot masquerade as current work.

Caching is optional. If introduced, keys include tenant, app/work, effective authorized scope and policy epoch, input/query versions and parameters. Source authorization is rechecked for cached/derived content; a saved summary is not a permission bypass.

## C07 Internal record mutation

A user submits a declared command with app/work/schema IDs, target record if any, expected record/work version, typed values and an idempotency key. The server derives actor and tenant; verifies active app/version, command permission, current membership, row/field restrictions and any referenced source; rejects undeclared fields, incompatible types and forbidden transitions.

After current authorization, check the idempotency outcome before applying the expected-version condition: a legitimate retry carries the original now-stale expected version. Same scoped key plus same canonical command payload returns the original outcome without another mutation; a reused key with a different payload conflicts. Scope keys to tenant/principal/work/command. For a new key, one transaction enforces uniqueness and the expected version, writes the record revision, stores the outcome and appends audit. Concurrent same-key requests serialize on that uniqueness boundary. The caller may retrieve only an outcome it is still authorized to see. Concurrent different edits use compare-and-set and return a conflict with an authorized recovery view, never silent last-writer-wins.

New mutation commit also checks the authoritative membership/policy epoch and saved-work/app/schema revision, serialized against acknowledged local revocation and version adoption. An HTTP-entry check alone is insufficient: if revocation or adoption wins that ordering, the stale command is denied/conflicted without a mutation. The full design must choose a transaction boundary or equivalent fenced commit protocol; an external authorization cache is not that guarantee. A committed command that won the ordering stays in authorized history; recheck current visibility before delivering its outcome.

Distinguish accepted, committed, failed and unknown-to-client states. After a timeout, retrieve the existing command outcome/retry with the same key; do not create a new intent. Retain the idempotency outcome for the declared retry horizon. After expiration, require explicit reconciliation before a new submission where duplicate effect matters. Cancellation before commit stops work; cancellation after commit reports the committed result rather than pretending it was undone.

Source-linked decisions and free-text notes inherit relevant source-access restrictions. A submitted “reviewed” value records an internal decision only. It does not publish the app, authorize a payment or cause an external action. All three meanings of approval are separate capability types.

## C08 Preview and transformation

The composer receives authorized schemas/catalog data and synthetic examples by default. Validate whole output before use; partial streams are not executable. A patch names base revision and target IDs, shows semantic changes, and conflicts if stale. Accepting a proposal creates a draft; undo creates another revision without erasing audit or committed work.

Preview defaults to synthetic input. Organization policy may enable real-data **read** preview for a validated immutable draft, available only to its author/authorized reviewer with their own current access and preapproved query operations. It has the same budgets/disclosure/result checks and is labeled unreviewed. Draft form writes always use an isolated preview record store; publishing does not copy preview records into live work. A preview result cannot become a finalized shared/exported decision without revalidation under a published version.

Schema changes are definition changes. A new field, enum constraint or relationship requires compatibility checking; destructive changes are blocked until an explicit migration plan exists. A mere label/layout patch does not replace unrelated work.

## C09 Publication policy and app lifecycle

Draft → In review → Published → Retired is visible. The server applies a versioned organization publication policy: an authorized owner-publish policy for scoped low-risk apps, or an independent-review policy such as Northstar's. This describes supported policy choices, not permission for authors to change policy. Administration of policy and access is a separate authority; AI cannot select its own grants or approvals.

Publication atomically binds exact definition/input-contract/record-schema/operation/renderer identities, policy, required approval(s) and audit. Input contracts describe required schemas and permitted operations, not the author's concrete evidence revisions; each new work binding selects compatible sources under its user's authority. Stale/replayed approval, changed content or revoked reviewer authority fails. Publishing capability declarations does not grant rights to actual sources. Existing published versions remain available during draft failures, except revocation/security restrictions.

## C10 Saved-work version adoption

New work starts from the current permitted publication. Existing work stays pinned; an authorized user explicitly adopts a compatible revision after seeing meaning/schema changes. Use a compare-and-set upgrade with a recorded source/target version, migration outcome and audit. Incompatible schema changes are rejected unless an approved deterministic migration is supplied. No background reinterpretation of previous findings.

Rollback chooses a still-permitted app revision and checks compatibility with the **current data schema**. It does not reverse prior record mutations or resurrect access. Record-history restore is a distinct authorized compensating command with current validation, not an app-version rollback side effect.

## C11 Sharing, access and revocation

Separate permissions for discover, run, source read, internal record read/write, edit definition, review/publish, share work, export and admin policy. Space membership can contribute but never automatically grants all of them. Links carry identifiers, not bearer access. App moves/links/forks and ownership changes re-evaluate audience/policy effects.

Saved findings, narratives, derived records and retained handoff previews require current access to the information they disclose. Scope includes their source dependencies, not only citation links. Deny/redact on revocation; explicit declassification would be a separate capability. Revocation prevents future server results, including queued/delayed ones, and invalidates active displays within the tested propagation target. Already delivered data and downloaded exports cannot be recalled.

## C12 AI authority and disclosure

Separate tools for composition, parameter changes, bounded authorized queries, result explanation and proposed record commands. First release requires user submission of record mutations; the model does not autonomously execute them. Publishing and policy changes are not composer tools.

Organization policy names the provider/model and allowed disclosure fields/size. Sensitive schemas, metadata, aggregates and low-cardinality groups count as data too. Real-data explanation is disabled until configured; approved apps and deterministic records/queries still operate. Model factual numbers must match actual query/record receipts; references do not certify a model's interpretation. Show uncertainty and retain human judgment.

Untrusted evidence may contain instructions. Neither retrieval nor a refusal prompt provides authority enforcement. The server allowlist, current authorization, validation and bounded execution enforce it. Do not log raw evidence into prompt/audit traces by default.

## C13 Library and application-specific meaning

The platform registers versioned trusted renderers/operators and validates their use generically. Northstar's connection schema, CIDR lookup, six-card arrangement, labels and caution about compromise are an app/starter definition plus registered domain capability. Spend Review uses generic table/detail/form/status components and integer currency semantics. Neither app requires a tenant-name or space-name branch in platform execution.

Creating a new app from already supported primitives requires no platform source change/redeployment. Adding a new primitive is an explicitly separate engineering release. Both should be explainable to the user without implying universal app generation.

## C14 External actions and other surfaces

Connected data is initially read-only; no external mail, payment, containment or source update is implicit. A future named action needs effect/authority/approval, idempotency scope, observed outcome, reconciliation and audit. A timeout may be uncertain; no universal exactly-once promise.

Browser is the first runtime. MCP Apps, a chosen live connector and customer model accounts are later adapters requiring their own identity/disclosure/host checks. Public websites are outside this release. Ordinary navigation, published queries and internal forms work during model outages.

## C15 Operations and retention

Durable work, quota/admission, cancellation, restore, deletion, model budget and operator responsibility are platform capabilities. Keep parser/query workloads constrained apart from privileged publication and credential contexts. Audit high-impact actions with actor, scope, revisions and outcome while minimizing payload data. Set concrete retention/support objectives before real customer data; demo values do not become universal company policy.

## C16 Demo compatibility

Both examples run from one platform build and the same schemas, admission, runtime, publication, query and record-command interfaces. Demo simplifications are deployment/volume/administration choices, each mapped to a platform extension. A second tenant canary tests boundaries. Simulated behavior is labeled and does not satisfy a working capability's acceptance gate. Record any contract amendment in both designs before implementing dependent behavior.
