# Forma company app platform product brief

## Product and intended outcome

Forma is a managed platform where teams turn data and intent into working apps, run them in their company workspace, and adapt and share them as their work changes. The maintained app and its saved work are the durable results. A chat, spreadsheet or generated screen can begin the process; none alone completes it.

The product has four distinct layers: Forma Stack supplies the framework; Forma supplies the company app platform; teams create task-specific apps inside it; demonstrations such as Northstar show what those apps can do. The launch product is the platform. Security is a rich example, not the identity imposed on every company or the predetermined first paying market.

My recommendation is to build this horizontal capability set while proving it through two bounded applications. Northstar supplies a meaningful investigation and reuse story. A compact Spend Review app exercises internal forms and record updates using the same creation path. The decisive claim is that a team can make the second app without a developer changing platform code for that example.

## Users and the recurring job

Builders may be operations practitioners, technical analysts or engineers who currently maintain spreadsheets, queries and small scripts. Their colleagues use the apps; designated reviewers apply company policy; administrators manage membership, sources, ownership and operating limits. Roles may overlap when policy permits. Northstar's Maya, Sarah and Jordan demonstrate responsibilities rather than a universal three-employee staffing requirement.

The recurring job is: help our team form a useful tool around its data and process, complete work inside it, share the appropriate result, and adapt or reuse the tool for the next job. A pilot needs usable data, a repeated need, a named owner and at least two people who can demonstrate continuation or reuse. Industry follows customer evidence; the demo does not decide it.

## Product model and navigation

| Concept | Platform meaning | Northstar example |
|---|---|---|
| Organization | Company membership, policy and isolation boundary | Northstar |
| Space | Company-defined organization of related apps/work | Security; Finance |
| App | Maintained tool with owner, definition and versions | Outbound Investigation; Spend Review |
| App version | Immutable admitted definition with declared inputs/operations | Published version 1 |
| Saved work | One continuing use, with bound sources, records, notes and context | A case; an October spend review |
| Canvas/view | Presentation of the app and current work | Six investigation cards; review queue/form |
| Source revision | Immutable selected input plus meaning/provenance | Log export; inventory worksheet; spending table |
| Internal record | Typed app-owned data entered or updated inside Forma | Review decision, assigned owner, note |

One app has one owning space and identity. Other permitted spaces may link to it; a deliberate fork creates a separate app identity. Changing ownership, space or audience is an explicit policy-bearing operation. A space theme is not an access grant.

Home shows permitted apps, recent work, spaces, owners, versions and input requirements. Clicking Security and entering `/security` resolve to the same stable space identity. Aliases are unique within an organization; old links use stable IDs. Partial input suggests permitted destinations, and navigation occurs on selection or Enter. Unknown commands neither create resources nor trigger actions. The command box does not intercept text in notes or formulas.

Opening an app offers **continue saved work** and **start new work** as distinct choices. Returning restores the relevant state under current access. Organization/space switching must not display stale results from the previous context. Shell navigation, normal filtering and ordinary record entry need no generation call.

## What teams can create in the first release

The reusable library supports summaries, tables, details, timelines, breakdowns/comparisons, filters, forms, notes and bounded review/status interactions. The canvas supports pan/zoom, move/resize, groups, fit-to-content and saved layouts. Focused tables/forms and keyboard-accessible navigation use the same app/work model; an infinite canvas is not compulsory for every task.

Teams can create and transform an app through direct controls or intent. Retrieval first finds permitted compatible tools; adaptation changes an existing definition; generation creates supported missing combinations, calculations, forms or rules. The product identifies which happened. A genuinely new comparison or persisted form is useful generation even when it uses an existing renderer. A new title or theme alone is not proof of a different app.

Proposed first representation: a versioned, typed `AppDefinition` describing views, queries, internal record schemas and bounded commands. Trusted first-party Forma components interpret it. The model does not emit executable code, SQL, FMIR, arbitrary HTML or permission grants. This is a design recommendation, not a requirement supplied by the PDFs. The factory should compare it with the historical admitted-markup approach and decide once for both designs.

Missing capability is an honest limit. A user can combine supported operators without platform code changes; adding a new trusted operator or renderer is an engineering release. The product must never disguise that work as generation already performed by the user.

## Data and work that persist

CSV and bounded tabular XLSX are first-release inputs. A visible drop zone and equivalent keyboard-accessible Upload control create a source card and inspection workflow, with limits shown before upload and progress/cancellation afterward. Imports expose worksheet selection, field mappings, row failures, units, timestamps and transformation history. New imports create new revisions. The product does not execute spreadsheet macros, arbitrary formulas or external links. Original evidence, normalized data and derived results follow explicit retention and deletion rules.

Connected source systems are initially read-only. **Records inside Forma are writable** through admitted forms and commands: responses, notes, ownership, review status and decisions. A saved internal decision is not an external payment approval, email or source-system update. Persistent fields are schema-validated and authorized on the server, with conflict and retry behavior the user can understand.

Personal layout and exploratory filters remain personal until deliberately shared. Committed work records and shared views have an explicit audience, further restricted by source access. Saving, sharing an app, sharing a work item and exporting a snapshot are separate operations. Reuse creates a new binding and work instance; it does not copy the author's data, decisions, credentials or authority.

## AI responsibilities

The composer proposes mappings, components, app changes and typed schemas using approved metadata and synthetic examples where possible. A separate result-explanation path can interpret authorized query receipts when organization data policy allows the provider to receive those results. Both are generic capabilities; Northstar can supply domain vocabulary and interpretation constraints.

AI cannot determine its own access or publish broader authority. Exact calculations and record mutations run through deterministic server operations. Generated explanations identify their supporting query/record revisions and uncertainty. Provider outage leaves published apps, deterministic queries and internal forms usable; genuinely model-dependent operations show their unavailable state.

Drafts use synthetic data by default. A policy may allow restricted real-data read previews after validation and with the preview user's own access. Draft form submissions use an isolated preview record store, never the live work's records. Material changes remain visible, reversible and separate from published behavior.

## Publication and the managed offering

Apps move through Draft, In review, Published and Retired. Publication policy belongs to the organization. Northstar requires Sarah to review Maya's shared changes; another low-risk configuration may allow an authorized builder to publish. The model never assigns that policy, self-selects a reviewer or grants broader source access.

A publication binds exact app, input-schema contract, operation, record-schema and renderer versions. Each work instance binds its own compatible evidence revisions. Existing work pins its app version; a compatible update is explicit. Changes to calculations or internal schemas require impact checks. Rollback does not roll back data changes or revive revoked rights. Owners can transfer maintenance, retire apps and explain missing dependencies.

The managed platform includes authenticated membership, company-private catalogs, operating limits, save/conflict states, audit, recovery, deletion and support ownership. Multiple organizations must be isolated in search, files, records, jobs, caches and administration before that offering is advertised. One Northstar recording is not evidence of production multitenancy.

## Scope and proof

First release: company home/spaces, click/slash navigation, real CSV/XLSX import, reusable views/forms, meaningful app generation and editing, published versions, source-linked queries/decisions, internal records, saved work, handoff/export and independent reuse. At least two different apps must exercise the common creation and execution path.

Later extensions follow customer need: one live connector, a bounded external action, a validated MCP Apps host, more operators and model options. Live coediting, scheduling, public websites, cross-company marketplace, arbitrary code execution, clinical decisions, autonomous remediation and a full accounting engine are not implicit launch obligations.

Measure the full workflow: preparation, creation, review, use, handoff, second-person reuse and maintenance. Also measure creation of another app without founder coding. Pricing and a team/organization subscription are hypotheses; there is no claimed traction, fixed reuse ratio or guaranteed performance advantage.

The pitch should open with Forma's company home and end with a second app living beside Northstar. That makes the platform visible before and after the rich example and prevents the audience from reasonably concluding that Forma is only a security dashboard.
