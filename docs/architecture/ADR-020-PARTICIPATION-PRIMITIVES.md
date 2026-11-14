# ADR-020 — Participation primitives (Release C)

Date: 2026-11-14. Owner: GenUI. Status: **accepted** (designed by Claude Fable 5.1 in lieu of Victor, under his instruction of 2026-11-14: "Go and build don't wait on my go remember fable can decide"; Victor can reverse any item). Tickets: APP-202 to APP-209. Contract edition: `impl-2026-11-14.2`. Release: **Release C**, a normal (non-schema) promotion after release B (schema 24, APP-194), about Nov 25, before the Nov 26 rehearsal. No deploy after Nov 28.

This ADR extends [ADR-007](ADR-007-PHASE-1-WRITE-AUTHORITY.md) (its 2026-11-14 amendment: own collections, reviewers, shares with roles), [ADR-002](ADR-002-CONTRACT-EDITION.md) (editions) and [ADR-011](ADR-011-NOV-2-EMERGENCY-PIVOT.md) (what a share grants; `share_work` is never copied). It changes none of their rules. Every numeric bound of ADR-004 and ADR-007 is unchanged. **There is no migration**: schema 24 is the schema Release C runs on.

## 1. Why

Victor's class (Nov 29–30, 25 adult students, his organization on beta) wants live quizzes, polls, homework and attendance. The platform stays general (AGENTS.md: "Both example apps must use the same general path without tenant-name branches"), so the class gets general primitives that Victor shapes with the composer. His words (2026-11-14): "we should be able to build participation tools/apps … It's easy to build a timer … tracking the answer the student wants to lock in, they can change and the change saves … radio buttons and a checkbox if multiple answers … a timer and or a global disable button … a public leaderboard will show stats not answers … we don't have to follow approver steps for students, that's too rigid, we have to be malleable."

The [APP-193 finding](../../evidence/APP-193/finding-2026-11-14.md) measured release B's primitives against a live quiz and found five gaps: one answer per person (G1), closing an item (G2), a reference picker (G3), a clock (G4) and a leaderboard (G5), plus the composer step (G7). Decision D11 of the [plan](../planning/TRIAL-25-READINESS.md) was NO-GO for Nov 29 on that basis. Release B then landed the private entries (D7), reviewer-only commands (D8), shares with roles (D9) and the refresh-herd fixes (APP-195, APP-197, APP-198, APP-200), which were the finding's preconditions. This ADR re-decides D11 as a bounded **go** for the primitives below, with release B's formative quizzes as the fallback (plan amendment of 2026-11-14, Release C).

## 2. Decisions, in one table

| # | Primitive | Grammar (edition `impl-2026-11-14.2`) | Where it is judged | Ticket |
| --- | --- | --- | --- | --- |
| C1 | **Countdown** | view kind `countdown` with `records: {version, input_id, schema_id, timestamp_field, offset_seconds?, closed_when?}` | the record-components path (one authorized snapshot under `own_rows`), the client ticks from `server_time` | APP-204 |
| C2 | **One entry per person per key** | `record_schema.one_per_creator: [field ids]` (requires `visibility: "own"`); a `create` whose key matches the creator's existing entry **replaces** it | `commands::commit` after the organization's commit-sequence lock; the dry run reports `replaces` | APP-203 |
| C3 | **Open while** | `command_declaration.requires: [guard…]`; a guard may carry `via` (a reference field, judged on the referenced record) and the literal `{"type": "now", "offset_seconds"?}`; `command_declaration.set_now: [timestamp field ids]` stamps server time | `commands::commit` and `dry_run_command`, same transaction instant as `committed_at` | APP-203 |
| C4 | **Leaderboard, phase 1** | `record_ranking.records.group_by: "creator"` (instead of `group_field`); measure `{"op": "matches", "key_input", "fields"}` on `record_ranking` and `record_summary`; ranking `limit` up to 50 | the record-components path under the viewer's `own_rows`; names from `memberships.display_name` | APP-204 |
| C5 | **Approver routing** | no grammar; the act draft carries per approver `standing: reviewer \| shareable \| unreachable` | the action, MCP and Slack paths; the canvas shares only `shareable` | APP-206 |
| C6 | **Multi-select (stretch)** | value type `{"type": "multi_enum", "values": [ids]}`, value `{"type": "multi_enum", "values": [ids]}` | forms (checkboxes), listings, rankings; refused in guards, filters, keys and typed queries | APP-208 |
| C7 | **Composer** | prompt grammar, one general example, admission tests; reference fields allowed when they target a records input of the same definition | `composer/prompt.rs`, `plan.rs` | APP-207 |
| C8 | **Release C** | the edition and the primitives above, one normal promotion | APP-209 | APP-209 |

Two new problem codes, both 422: `requirement_not_met` (a `requires` guard failed) and `unchanged` (a replacing create found the entry already holds these values). A reference picker in forms (G3) is part of C3's delivery (APP-205), because without it no member can point at an item.

Each meaning has exactly one encoding: every new member is optional, absence keeps today's meaning, and the explicit alternatives (`group_by` beside `group_field`, `set_now` on a writable field, a `now` literal outside `requires`, `one_per_creator` on a shared list, an empty `multi_enum` selection) are refused rather than given a second meaning. Older definitions stay valid unchanged; every retained fixture keeps its verdict and digest.

## 3. Shared vocabulary

**Server time.** Inside a command transaction, `now` is the transaction's `now()` truncated to the whole second: the same instant `definitions::fresh` already stores as the revision's `committed_at`. PostgreSQL fixes `now()` for the whole transaction, so every guard, every `set_now` stamp and the stored `committed_at` of one commit agree, and a refusal or acceptance can be re-derived from stored data afterwards. The record-components route reports `server_time` (whole-second UTC, taken by the binary when it builds the response) so that a browser's clock never decides a countdown.

**The current record of a records input** (used by C1 and by the reference picker's default choice, nowhere else): among the records of the input's bound collection that the viewer may read (`own_rows`), the one whose current revision has the highest `commit_sequence`; none when the viewer can read none. This is "the item most recently touched": posting an item makes it current, and so does closing or reopening one. The plan's class guidance therefore says: close the open item before posting the next. (Alternative considered: "most recently created". Rejected, because reopening an item would not make it current, and because it needs the first revision's sequence, which no read path carries today.)

**Typed equality.** Two values are equal when their canonical forma-json-v1 bytes are equal (the `{"type", …}` object, every member). `null` equals `null`.

**Refusal vs denial.** A `requires` guard that fails on a record the committer may read is a typed refusal (`requirement_not_met`, 422). A referenced record the committer may not read, or that does not exist, is `Denied` (the uniform 404), as any unreadable record is today, so a guard never discloses another member's row.

## 4. C1: countdown

**Grammar.** A view of kind `countdown` is a records view: no `query`, `command_id` or `text`; `records` is `{version: 1, input_id, schema_id, timestamp_field, offset_seconds?, closed_when?}`.
- `timestamp_field`: a field of the schema of type `timestamp`.
- `offset_seconds`: an integer in −604800..604800 (one week), default 0. The target instant is `timestamp_field + offset_seconds`.
- `closed_when`: a guard `{field_id, comparison, value}` over the same schema (APP-107 rules: a typed non-null literal of the field's type, ordering only over ordered fields; no `via`, no `now`).
- Refused: a `timestamp_field` of another type, a `closed_when` naming another schema's field or carrying `via`/`now`, a `query`/`command_id`/`text` beside `records`, an input whose fields differ from the schema's (the existing records rule).

**Semantics.** The component is computed by `record_engine.py` over the authorized snapshot like every record component: the current record (§3) of the input; `value` is the target instant as a contract timestamp (`YYYY-MM-DDTHH:MM:SSZ`) or `null` when the field is null; `label` is the current record's first `text` field value (at most 200 characters) or `""`; `closed` is `true` when `closed_when` holds on the current record or `value` is null; `state` is `empty` when there is no readable record. The wire result gains the top-level `server_time` for every record-components response, and the projection gains the optional `label` and `closed` members. The client computes `remaining = target − (server_time + elapsed since the response)` and shows `m:ss` (or `h:mm:ss` from one hour), "Closed" at zero or when `closed`, and "Nothing is open yet" when empty. It re-runs when the card re-runs (a `record.committed` hint for the work, APP-191's delivery rule, or the 30 s page timer): no new event kind.

**Authorization and D7.** The snapshot is read under `own_rows`, so a countdown over an `own` list shows each member their own current entry and reviewers the collection's. A guest sees it only for a shared collection (a live share of a work with an `own` collection is refused, ADR-007). The receipt covers it: the receipt kind's `components[].kind` enumeration gains `countdown`, and the receipt's `snapshot` lists the input's revisions as for any component.

**Failure modes.** No readable record: "Nothing is open yet". A null timestamp: "Closed". A snapshot bound exceeded: the component's existing `failed` state. Clock skew: irrelevant to fairness, because acceptance is judged by C3 on the server; the display may be off by the network delay of one fetch (under a second on beta), which the plan accepts.

**Composer.** May emit a countdown over a shared list whose items carry a `set_now` timestamp (C3), with `offset_seconds` equal to the stated duration, and a `closed_when` over the state field when the app has a close command.

## 5. C2: one entry per person per key

### Two candidate shapes

**A. On the record schema: `one_per_creator: [field ids]`.** The collection carries the invariant: for each creator, at most one record per key tuple. Every `create` on it honors the rule; a create whose key matches the creator's existing entry replaces that entry.

**B. On the command: `operation: "replace"` (or `replace_own: [field ids]`).** Each command says whether it replaces.

**Comparison.** A is one declaration that every path (canvas form, Slack, MCP, the action path, a later command the composer adds) honors without each command restating it; a listing can show "your entry per item"; the composer has one place to put it and cannot produce a collection where one form replaces and another appends. B lets two commands disagree about the same collection, which is a second meaning for the same rows, and it names an operation the APP-013 rules (create creates; update needs an existing version) would have to special-case on the wire. **Chosen: A.** B's only advantage, replacing through a command that is not a create, is not needed: an `update` on a keyed collection is still an ordinary update of a record the member can see.

### Grammar

`record_schema.one_per_creator`: an array of 1..4 distinct field ids of the schema. Validation (Rust admission and `validate.py`, both):
- the schema declares `visibility: "own"` (a per-creator key on a shared list is refused: the rule is about private entries, and a shared list's "one per person" would need a different display);
- no key field has type `multi_enum` (set equality is not a key);
- every `create` command on the schema lists every key field in `writable_fields` (the filer always chooses the key);
- no `update` command on the schema lists a key field in `writable_fields` (a key never moves; the creator files again instead);
- an empty array is refused (`minItems` 1).

### Storage semantics (`commands::commit`)

The rule applies to a `create` command whose requested record is absent (`expected_record_version` 0 and no row), on a schema with `one_per_creator`. Order inside the transaction, unchanged up to the premise: precheck → claim → fence → reviewer read → work fence → published declaration → collection → **organization commit-sequence `FOR UPDATE`** → the requested record row `FOR UPDATE` (absent) → the version premise → `compose` as a create (nulls, `initial`, `decided`, `set_now` stamps) → `requires` (C3) → **the key match**:

1. The key values are taken from the composed state (so `initial` and `set_now` never form part of a key, and a null key value is a key like any other).
2. One statement finds the creator's own current revision in the collection whose canonical values contain the key pairs, excluding the requested record id, newest first, locked:

   `SELECT r.record_id::text, r.current_version::bigint, v.canonical_values FROM genui.internal_records r CROSS JOIN LATERAL (…current revision…) v WHERE r.organization_id=$org AND r.app_id=$app AND r.work_id=$work AND r.collection_id=$collection AND r.record_id<>$requested AND v.creator_id=<SCOPED> AND <own_rows(v)> AND convert_from(v.canonical_values,'UTF8')::jsonb @> $key::jsonb ORDER BY v.commit_sequence DESC LIMIT 1 FOR UPDATE OF r`

   `$key` is the forma-json-v1 array of `{field_id, value}` pairs of the key fields; jsonb array containment matches each pair against one element of the stored `{field_id, value}` array, and object containment on the typed `value` object is exact for every scalar type (the key may not be `multi_enum`, whose array would match as a subset). The statement reads only the creator's rows, carries `own_rows` like every revision read (the scan test `every_record_statement_carries_own_rows` keeps it), and takes the matched row `FOR UPDATE` after the sequence lock, in the D7.9 order. `convert_from` and the `jsonb` cast are built-in; no definer and no grant changes.
3. **No match:** the create proceeds exactly as today (version 1 of the requested id).
4. **A match (record Y at version n, state S):** if S equals the composed state, the command is `Refused(Unchanged)` and the transaction rolls back (the claim is released; a later identical submission is judged again). Otherwise the commit writes revision n+1 of Y with the composed state, `creator_id` = Y's creator (the principal, by construction), `updater_id` = the principal, the current-version pointer with the `n` premise, the source dependencies, one `command.committed` event naming Y, the advanced sequence and the outcome naming **Y at n+1**. The requested record id is not created. The request's `expected_record_version` was already judged against the requested id, so a stale page never answers `Conflict` here: the latest submission wins, which is the "locked in" meaning Victor asked for.
5. A replace never fires a transition (a create composes fresh), never takes `by_filer`'s decision rule, and the `decided` fence applies to the composed state as for any create.

**Concurrency.** Every commit of the organization queues on `organization_sequences FOR UPDATE` (D7.7), so two submissions by one creator for the same key are serialized before either looks for a match; the second finds the first's row and replaces it. The matched row's `FOR UPDATE` after the sequence lock is defensive (the same lock order as the requested row). Two creators never match each other's rows (`creator_id = SCOPED`). No index is needed: the scan covers one creator's rows of one collection, bounded by the number of distinct keys that creator has filed; the statement stays inside ADR-004's 2 s budget at any plausible class size (25 members × 50 items).

**Idempotency.** Unchanged: the claim is keyed on organization, principal, app, work, command and key, and a replay of the same form returns the stored outcome (which names Y). A fresh form mints a fresh key and a fresh requested id; the rule, not the id, decides whether a row is added.

**Dry run.** `Data::dry_run_command` runs the same match without `FOR UPDATE` and returns `DryRun::Valid { …, replaces: Option<String> }`; the confirmation card may say "This replaces your earlier entry". The Slack and MCP commit paths go through `commit_record`, so they get the rule for free.

**D7, D8, live hints, summaries.** Unchanged: the written revision belongs to an `own` collection, so its hint reaches its creator, the reviewers and the owner only (`openable_principals`); summaries, rankings and listings read current revisions, so a replaced entry counts once. A reviewer's `reviewer_only` decision on the entry (an `update` or a transition `create`) is unaffected: it targets the existing record id.

**Failure modes.** Stored duplicates predating the rule cannot exist for a collection whose schema declares it (a work pins its publication; adoption is operator-only, D7.4). Defensively, the newest match wins and older rows are untouched. A key field left null by a nullable schema is a valid key (one "no item" entry per creator). A match statement past the budget is `Unavailable` (503), nothing written.

**HTTP.** `CommandResult::Refused(Refusal::Unchanged)` → 422 `{"code": "unchanged"}` (JSON mode); the record page's form mode keeps `Notice::Refused`. Client copy (`commands.ts`): "Already locked in: your entry already holds these values."

**Composer.** Writes `one_per_creator` with the field that names the item (the reference field, C3) when the member says each person answers or submits once per item, question, assignment or day and may change it; otherwise omits it.

## 6. C3: open while

### Two candidate shapes

**A. Command-level `requires: [guard…]`, guards that may look through a reference (`via`), and a server-time literal.** "Accept this command only while these guards hold", each guard over this record or over the record one of its reference fields names; `now` is a literal that only a `requires` guard may carry; `set_now` stamps server time into timestamp fields the form cannot write.

**B. Schema-level `open_while: {via, state_field, open_value, timestamp_field, seconds}`.** One fixed shape: "entries are accepted while the referenced record's state is open and within N seconds of its timestamp".

**Comparison.** A is the APP-107 guard grammar with two additive members, judged by the one `rules.rs` path storage already has; it also expresses "check in within 10 minutes of the session's start", "submit only while the assignment is open", "a response needs the item's prompt to be set", with or without a clock, with or without a stop button. B is a quiz-shaped composite that fixes which fields exist and how the window is computed, and it cannot say "open, no timer" or "timer, no state" without two more variants. B is marginally easier for the composer, but the composer gets one worked example either way. **Chosen: A.**

### Grammar

- `command_declaration.requires`: an array of 1..4 guards, on `create` or `update` commands. A guard is `{field_id, comparison, value, via?}`.
  - Without `via`: APP-107's rule over this record schema (`judge_guard`), plus: the literal may be `{"type": "now", "offset_seconds"?}` only when the field is a `timestamp`.
  - With `via`: `via` names a field of this schema of type `reference` whose `target_input` is a `records` input of this definition; `field_id` names a field of that input's `fields` (the input and its record schema have identical fields by the existing rule); the literal is typed against that field (`compatible`), ordering comparisons need an ordered field, and `now` needs a `timestamp` field.
  - A `now` literal anywhere else (a query filter, a transition's `when`, an `initial` guard, a `closed_when`) is refused. `offset_seconds` is an integer in −604800..604800; absence means 0.
- `command_declaration.set_now`: 1..4 distinct field ids of the schema, each of type `timestamp`, none in `writable_fields` (one source of a field's value), on `create` or `update`.
- A `multi_enum` field (C6) may not be a guard's field.

### Storage semantics

`Declaration` gains `requires: Vec<Guard>` (with `via`) and `set_now: Vec<String>`; `rules::Guard` gains `via: Option<String>` and `holds_at(state, now)`, where `now` is the transaction instant (§3). `compose` gains the instant and stamps every `set_now` field with `{"type": "timestamp", "value": now}` before the nullable check (so a non-nullable stamped field composes on a create). A storage function `judge_requires(transaction, org, app, work, declaration, composed) -> Result<()>` runs after `compose`, before C2's match and before `copy_capacity`, on the commit and the dry run alike:

1. Without `via`: `holds_at(composed, now)`.
2. With `via`: the composed state's value of the `via` field is `{"type": "reference", "input_id": I, "record_id": R}` (null satisfies no guard → refusal). The work's binding of input `I` (`work_bindings`, kind `records`, `committed_snapshot`) gives the collection; `R`'s current revision is read with `current_state(…, lock: false)`, which carries `own_rows` and answers `Denied` for a row the committer cannot read or that does not exist. The guard is judged on that revision's state with `holds_at(state, now)`. The referenced row is **not** locked: the organization's sequence lock already serializes every commit, and the read sees committed state as of the statement, so a close that committed before the answer's transaction reached the sequence lock is seen, and one that commits after is not (the answer landed first). That is the fairness bound: an answer is late only if the close committed first.
3. Any failing guard: `CommandResult::Refused(Refusal::RequirementNotMet)`, transaction rolled back, nothing written, the claim released.

**The stop button.** A `reviewer_only` `update` that sets the item's state to `closed` (not a declared transition, so the owner can fire it on their own item; the APP-113 rule at `commands.rs:887` is unchanged). The next answer's `requires` guard `{via: item, field_id: state, comparison: eq, value: open}` fails. The timer is the second guard `{via: item, field_id: opened_at, comparison: gte, value: {type: now, offset_seconds: -30}}` ("opened within the last 30 s"), with `opened_at` stamped by `set_now` on the post and on the reopen command. Either guard alone is valid.

**Interaction with D7, D8, live hints.** The referenced read carries `own_rows` (a shared item is readable by every contributor; a reference into an `own` list resolves only for the row's creator and reviewers). `reviewer_only` is judged before any value, as today. `set_now` on a `reviewer_only` update re-stamps the item, which is a commit to a shared list and reaches every open page by the existing delivery (the plan's class guidance on one change at a time stands). Summaries are unchanged. A guest cannot commit.

**Authorization.** No new permission. The committer needs `record_write` (as today), reads the referenced record under their own `own_rows`, and the judgment adds no write.

**Failure modes.** A reference to a record of another work or collection: the value's `input_id` must equal the field's `target_input`, and the binding lookup is by this work, so a foreign id is simply not found → `Denied`. A reference input bound to nothing (a work whose binding is missing): `Denied`. A `set_now` update that changes nothing but the stamp within the same second: refused as a no-op revision (the existing rule); the next second it writes. Clock changes on the database host move every judgment together.

**HTTP.** `Refused(RequirementNotMet)` → 422 `{"code": "requirement_not_met"}`; client copy: "Not saved: the item this refers to is no longer open." Form mode: `Notice::Refused`. The dry run's refusal is the action path's `could_not_draft` with the same text, and MCP/Slack carry the code in their typed replies.

**Composer.** May emit `requires` with `via` and `now`, and `set_now`, for "answers count only while the item is open" and "closes after N seconds"; it never writes `now` elsewhere. It must give the item list a reviewer-only create (with `set_now`) and a reviewer-only update that closes (and optionally reopens with `set_now`), both without declared transitions (the existing OWNER-ADVANCED STATE rule).

### The reference picker (G3; APP-205)

A form field of type `reference` whose `target_input` is bound in the work is rendered as a select of the target collection's records the member may read (the work's listing is already read under `own_rows`): option value = record id, label = the record's first `text` field value (at most 100 characters) or "Record", newest first by commit sequence, the current record (§3) preselected; at most 200 options. The canvas form, the record page's form and the FMIR SSR form all use the existing `has_choices` rendering; `typedFormValue` and `form_typed_value` already produce `{"type": "reference", "input_id", "record_id"}`. A reference whose target input is not bound, or a target the member can read nothing of, renders a disabled select with "Nothing to choose yet". `set_now` fields render read-only, as non-writable fields do today. The composer's "no reference fields" refusal is narrowed to references whose target is not a records input of the same definition (C7).

## 7. C4: leaderboard, phase 1

**Scope.** A ranking by person with a score, on the page of a reviewer or the owner (Victor projects it). Students' devices never receive other students' data. **Phase 2**, a published-stats view on students' devices, is post-class: it needs an ADR-007 exception to D7 and a consent-notice change, and it is only named here.

**Grammar.**
- `record_ranking.records` requires `measure` and **exactly one** of `group_field` (as today) or `group_by: "creator"`. `group_by` admits only `"creator"`. Both present, or neither, is refused.
- `record_measure` gains `{"op": "matches", "key_input": ID, "fields": [1..4 distinct ids]}`, admitted on `record_summary` and `record_ranking`: `key_input` is a `records` input of the definition other than the component's `input_id`; every id in `fields` exists in the component's schema and in the key input's fields with identical `value_type`; no field is `multi_enum`.
- `record_ranking.limit` is 1..50 (was 20); other kinds keep their bounds.

**Semantics.** The engine (`record_engine.py`) receives, per dataset row, `creator` (the principal id of the row's creator) and, at the top level, `creators`: a map from principal id to the label `directory::label(organization, principal)` (the member's `display_name` from release B, else the neutral "Member 1a2b…"). `group_by: "creator"` groups by `creator` and labels each group with its name; `matches` counts the rows whose values on `fields` equal (§3 typed equality) the values of some row of the key input's dataset on the same field ids. The key input is captured like any other input of the work (its revisions appear in the receipt's `snapshot`). Groups sort by value descending, then label; ties share the order of their labels. The snapshot digest domain becomes `forma:record-components-snapshot:v2` (the rows now carry `creator`); the receipt kind, its schema and the `forma:record-components-receipt:v1` domain are unchanged, because the receipt lists revisions, not rows.

**Authorization and D7 (the point).** The capture reads both inputs under the viewer's `own_rows`. A reviewer or the owner reads every response and every key row, so the ranking is complete. A contributor reads only their own responses and none of the owner's `own` key list, so their page computes a ranking of one row (themselves) with a score of 0, and the live hints they receive are only for their own rows (APP-191). No new authority path, no released aggregate, no change to delivery: D7's "same rule on every path" holds verbatim. The plan places the ranking card on the reviewer page; if the shared app also shows it, a student sees only their own name and zero, which is honest and harmless.

**Failure modes.** A key list with no rows: every score is 0. A creator with no display name: the neutral label. More than 50 members: the ranking is truncated (`truncated_page`) and says so. A name longer than the group label bound (2,000): impossible (`usable_name` bounds it at 80).

**Composer.** Emits a ranking by creator with a `matches` measure against an `own` key list that only the owner files (a reviewer-only create), and places it in a section the owner opens; it never promises students a ranking.

## 8. C5: the approver step for filers without share authority

### Today

`canvas/actions.ts` `shareWith` posts `/v1/work/{app}/{work}/share/add` for each approver the member's sentence named (resolved by `actions::resolve::match_approvers` from the members list). The route judges the **filer's** share standing (ADR-011: the work's unrevoked creator or a `share_work` holder, with `run`); a student in Victor's app is neither, so the share is the uniform 404 and the card says "The share with X was refused; the record is filed." MCP (`mcp/tools.rs`) and Slack share the same way.

### Two candidate rules

**A. Server-computed standing, no new write.** The draft names, per approver, `standing`: `reviewer` (the approver is a reviewer of the work: the owner, the work's unrevoked creator, a `record_review` holder, or covered by a live organization-wide reviewer share, `authorization::reviewer_clause`), `shareable` (not a reviewer, and the filer holds share authority for a contributor share: today's `share_standing`), or `unreachable`. The canvas shares only `shareable`; for `reviewer` it notes "Filed for X's review" without a request; for `unreachable` it notes "X is not a reviewer of this app; the record is filed and visible to its reviewers."

**B. Routing under the owner's authority.** A new definer, `genui_private.route_to_approver(org, filer, app, work, grantee)`, grants the approver `record_read` (and `record_write`, for a decision) on the work "because the owner's definition named the approvers", bypassing the filer's standing.

**Comparison.** The definition names no approver: `AUTHORITY_WORDS` refuses `approver` as a key, the approver comes from the filer's sentence, and ADR-011 says a share copies the sharer's own permissions and `share_work` is never copied. B therefore lets a filer with no authority widen another member's access on their word, under the owner's name, and it needs a migration (a new `genui_admit` definer, D7.1), which Release C cannot carry. A's "owner's authority" is the owner's existing designation of reviewers (D7, D9): a reviewer can already see the record (`record_read` on the work and the reviewer rule), so no share is needed, and nobody else can be reached by a filer who cannot share. **Chosen: A.** Exactly who may be "routed to": the app's owner, the work's unrevoked creator, a holder of `record_review` on the work, and every active member while a live organization-wide reviewer share exists. Nobody else, ever, from a filer without share authority.

**Where.** `storage::Workspace::approver_standing(request, app, work, grantee) -> Standing` (a lock-free read in `storage/sharing.rs`: the reviewer clause for the grantee, then the filer's `share_standing`); `actions::resolve` fills `standing` per approver; `actions/routes.rs` serializes it; `canvas/actions.ts` branches on it; `mcp/tools.rs` and `slack/flows.rs` call `share_work` only for `shareable` and word the rest. The share routes and definers are unchanged.

**Authorization.** The read discloses only whether a member the filer already named from the members list is a reviewer of a work the filer can write to: the same fact the share dialog shows a sharer, and a fact a filer learns anyway when a reviewer acts. No write, no new permission, no widening.

**Failure modes.** The standing is read before the commit and acted on after it; a reviewer grant revoked in between leaves a note that is one commit stale, with nothing written on its account. A `shareable` share that then fails keeps today's "could not be sent; the record is filed".

## 9. C6: multi-select (stretch)

**Grammar.** Value type `{"type": "multi_enum", "values": [1..32 distinct ids]}`; value `{"type": "multi_enum", "values": [ids]}`, a non-empty subset of the declared values **in declaration order** (any other order, a duplicate, an unknown id or an empty array is refused; "none selected" is `{"type": "null"}`, so a required multi-select is a non-nullable field). `compatible` checks the subset and the order. Refused: a `multi_enum` field in a guard, a typed query `filter`, a `record_filter`, a `one_per_creator` key, a `matches` field, a `records_list` `order`, or a transition.

**Semantics.** Forms render checkboxes (one per declared value; the client composes the ordered array; the SSR form reads the repeated input name). `records_list` cells show the values joined by ", ". `record_ranking` over a `multi_enum` `group_field` counts a row once per selected value ("each option's count"). The typed query snapshot (`record_query::scalar_row`) projects the field as text joined by ", " so existing engines see a string. Summaries by `count` are unchanged.

**Composer.** Emits `multi_enum` for "choose all that apply", only after APP-208 lands; until then the planner refuses it in generated content (`unusable_field`) while admission accepts it in stored content, so the edition is published once.

**Cut rule.** APP-208 starts only after APP-203, APP-204 and APP-205 merge; if it is not merged by Nov 21 noon UTC it is out of Release C and the composer is not told about `multi_enum`.

## 10. C7: what the composer may emit

- `visibility: "own"`, `reviewer_only: true` (release B), `one_per_creator`, `requires` (with `via` and `now`), `set_now`, `countdown`, `record_ranking` with `group_by: "creator"`, `matches`, and (after APP-208) `multi_enum`.
- A `reference` field only when its `target_input` is a `records` input of the same definition (the picker renders it); never a reference to a source.
- Never: `now` outside `requires`; `one_per_creator` on a shared list; `set_now` on a writable field; a declared transition for an owner's advance/close; anything about permissions beyond the grammar members; `visibility: "shared"`, `reviewer_only: false`, `group_by` beside `group_field`.
- One general worked example (`participation_example()`): a shared `items` list (`prompt` text, `state` enum open|closed, `opened_at` timestamp) with a reviewer-only `post` (create, `set_now: ["opened_at"]`, `initial` state open) and reviewer-only `close`/`reopen` updates (no transitions; `reopen` re-stamps); an `own` `responses` list (`item` reference to `items`, `choice` enum a|b|c|d) with `one_per_creator: ["item"]` and a `respond` create whose `requires` are the two guards of §6; an `own` `expected` list (`item`, `choice`) with a reviewer-only create; views: the `respond` form, a `countdown` over `items` (`opened_at` + 30, closed when state = closed), a `records_list` of items, a `record_summary` of responses, and a `record_ranking` by creator with `matches` against `expected`. No classroom words: items, responses, expected, choice.
- Acceptance intent (the sentence the ticket must generate from): "a quiz where each student answers once per question and can change it until the question closes after 30 seconds, with a leaderboard on my screen." The words "quiz", "student", "question" are the member's; the generated ids are general.

## 11. Contract edition `impl-2026-11-14.2`

Opened by APP-202 for this ADR. Edition `impl-2026-11-14.1` is retained whole under `contracts/fixtures/impl-2026-11-14.1/` with its identities in the lock; every retained vector and fixture keeps its verdict and digest. No document kind, `schema_version`, digest domain registered in the contracts, or problem code of an earlier edition changes.

| Change | Classification |
| --- | --- |
| `record_schema.one_per_creator` (optional, 1..4 ids) | widens compatibly; the §5 rules apply only when present |
| `guard.via` (optional); the literal `{"type": "now", "offset_seconds"?}` admitted only inside `command_declaration.requires[].value` | widens compatibly; `now` elsewhere is refused |
| `command_declaration.requires` (optional, 1..4 guards), `command_declaration.set_now` (optional, 1..4 ids) | widens compatibly |
| renderer kind `countdown`; `record_component` members `timestamp_field`, `offset_seconds`, `closed_when` for it | a new registered kind at renderer version 1; the publication's `renderers` list may name it |
| `record_component.group_by: "creator"` (xor `group_field`); `record_measure` `matches`; ranking `limit` 1..50 | widens compatibly |
| value type and value `multi_enum` | widens compatibly; refused where §9 says |
| problem codes `requirement_not_met`, `unchanged` (422) | additive |
| `record_components_receipt.components[].kind` gains `countdown` | additive |
| `schema.json` `$id` → `…/impl-2026-11-14.2/schema.json` | identity |

Negative vectors (at least fourteen, authored; Rust admission reproduces each verdict): `one_per_creator` on a shared schema; a key field absent from a create's `writable_fields`; a key field in an update's `writable_fields`; a `multi_enum` key; `via` naming a non-reference field; `via` whose target is a source input; `now` against an enum field; `now` in a query filter; `now` in a transition's `when`; `set_now` on a writable field; `set_now` on a date field; `group_by` beside `group_field`; `group_by: "field"`; `matches` with a field of different types; `matches` with `key_input` equal to `input_id`; `countdown` with a `query`; an empty `multi_enum` value; a `multi_enum` value out of declaration order; a 422 body with code `unchanged` carrying a `recovery_view`.

Both validators (Rust `admission::validate`, Python `validate.py`) enforce every rule above on stored and new content alike (these are shape rules, not APP-113 authoring rules).

## 12. No migration: what was checked

| Need | Why no migration |
| --- | --- |
| Finding a creator's entry by key (C2) | one creator's rows of one collection, matched by built-in jsonb containment; no index, no definer |
| Server time (C3) | the transaction's `now()`, already stored as `committed_at` |
| Reading a referenced record (C3) | `work_bindings` and the current revision, under `own_rows` |
| Creator names (C4) | `memberships.display_name` (migration 0023) through `directory::label` |
| Grouping by creator (C4) | `internal_record_revisions.creator_id`, already read |
| Approver routing (C5) | a read of existing clauses; no grant |
| Refusal codes | wire only |

Rejected because they need a migration: a unique index on `(collection, creator, key)` (and a generated key column); a definer that shares on the owner's behalf; a stored `server_time` column; a per-work "current item" pointer table. None is required for the class; each is a candidate for a later schema release if measurements ask for it.

## 13. Build partition

Builders own disjoint files. The grammar builder goes first and publishes the shapes; the others start from its merged branch (or from its published grammar text on day one, rebasing when it merges). A shared file changes only in its owner's branch.

| Builder | Ticket | Owns (exact files) | Publishes / consumes |
| --- | --- | --- | --- |
| **B0 grammar** (first) | APP-202 | `contracts/schema.py`, `contracts/schema.json`, `contracts/validate.py`, `contracts/make_fixtures.py`, `contracts/record_components_contract.py`, `contracts/test_contracts.py`, `contracts/fixtures/negative-vectors.json`, `contracts/fixtures/impl-2026-11-14.1/**` (new, retained copy), `contracts/implementation-lock.json`, `contracts/README.md`, `app/src/admission/validate.rs`, `app/src/admission/tests.rs`, `app/src/admission/record_receipt.rs`, `app/src/admission/mod.rs`, `docs/architecture/ADR-002-CONTRACT-EDITION.md` (edition note) | Publishes §§4–11's grammar as admitted shapes, `admission::RENDERERS` (+`countdown`), `RECORD_KINDS`, the problem codes. Consumes nothing. |
| **B1 storage rules** (Tier A) | APP-203 | `app/src/storage/commands.rs`, `app/src/storage/rules.rs`, `app/src/storage/actions.rs`, `app/src/storage/mod.rs`, `app/src/storage/tests.rs` (scan allowlist), `app/src/storage/participation_campaign.rs` (new), `scripts/dev/campaigns.tsv`, `app/src/commands/routes.rs`, `app/admin/src/canvas/commands.ts` | Publishes `CommandResult::Refused(Refusal)` with `Refusal::{RequirementNotMet, Unchanged}`, `DryRun::Valid { replaces: Option<String>, … }`, HTTP 422 `{code}`, the client copy. Consumes B0's grammar (parses stored JSON in `rules.rs`). |
| **B2 components** (Tier A for D7) | APP-204 | `app/src/storage/record_components.rs`, `app/src/storage/record_components_campaign.rs`, `app/src/query/record_components.rs`, `app/src/query/record_routes.rs`, `app/runtime/record_engine.py`, `app/runtime/test_record_engine.py`, `app/admin/src/record-components/*` | Publishes the wire `server_time`, projection `label`/`closed`, the engine payload `creator`/`creators`, domain `forma:record-components-snapshot:v2`, `is_record_kind("countdown")`. Consumes B0 (receipt kind, grammar) and `directory::label` (read only). |
| **B3 forms** | APP-205 | `app/src/query/routes.rs`, `app/src/query/canvas.rs`, `app/src/query/page.rs`, `app/src/commands/mod.rs`, `app/src/main.rs` (the SSR form), `app/admin/src/work/render.ts`, `app/admin/src/canvas/render.ts`, `app/src/query/canvas_tests.rs` | Publishes the reference picker (choices on `commands::FieldRow`, value = record id, label = first text), read-only `set_now` fields. Consumes B0; posts the same `command_request` B1 judges. |
| **B4 approver routing** (Tier A for authorization) | APP-206 | `app/src/storage/sharing.rs` (one read: `approver_standing`), `app/src/actions/resolve.rs`, `app/src/actions/routes.rs`, `app/src/actions/mod.rs`, `app/src/actions/tests.rs`, `app/admin/src/canvas/actions.ts`, `app/src/mcp/tools.rs`, `app/src/mcp/tests.rs`, `app/src/slack/flows.rs`, `app/src/slack/tests.rs`, `app/src/storage/actions_campaign.rs`, `app/src/storage/mcp_campaign.rs`, `app/src/storage/slack_campaign.rs` | Publishes draft `approvers[].standing`. Consumes nothing new. |
| **B5 composer** | APP-207 | `app/src/composer/prompt.rs`, `app/src/composer/plan.rs`, `app/src/composer/tests.rs`, `app/src/storage/composer_qualification.rs` | Consumes B0 (admission of the example); its example must run end to end on B1–B3's merged branch for acceptance. |
| **B6 multi-select** (stretch, after B1–B3 merge) | APP-208 | `app/src/storage/record_query.rs`, plus, after their merges, B3's form files and B2's engine files for the checkbox and the per-option count | Consumes B0. |
| **B7 release** | APP-209 | `docs/planning/TRIAL-25-READINESS.md`, `docs/operations/TRIAL-RUNBOOK.md` (a Release C section), `evidence/APP-209/**`, the promotion record | Consumes everything merged. |

**Interfaces, stated once.**
- B0 → all: the JSON grammar of §§4–11; the only coupling is the stored canonical content.
- B1 → B3: the 422 bodies and the `DryRun.replaces` member; B3's forms post today's `command_request` shape unchanged.
- B2 → client: `server_time`, `label`, `closed` on the record-components result; nothing in B3's files.
- B4 → client: `standing` on each draft approver.
- B5 → B1/B2/B3: nothing at build time; the acceptance test is the example app admitted, published and exercised on the merged branch.
- Every builder: `just check` plus its campaign; a Tier A review file in `evidence/APP-20x/` for B1, B2, B4; B0 adds the contract checks (`just check contracts`).

**Calendar.** B0 Nov 15–16 (merge by Nov 16 evening UTC). B1–B5 Nov 16–20 in parallel, Tier A reviews Nov 20–21, merges Nov 21–22 (after release B's branch merges to `main`, so Release C's PRs target `main` after schema 24). B6 only inside its cut rule. Release C promotion about Nov 25 after release B (Nov 23) is read back; Victor's build day moves its participation apps to Nov 25–26; rehearsal Nov 26.

## 14. Security review points (for the Tier A reviewers)

1. `one_per_creator` matches only `creator_id = SCOPED` rows and carries `own_rows`; a bypass makes the private-entries style check fail (the campaign runs each check with `authorization::BYPASS` set).
2. The `via` read is `current_state` with `own_rows`; an unreadable referenced record is `Denied`, never `requirement_not_met`.
3. `now` cannot be supplied by a client: the literal is a declaration member, the instant is the transaction's.
4. The ranking by creator computes under the viewer's `own_rows`; a contributor's page gets at most their own row; the result document carries labels, not principal ids; the receipt lists revisions only.
5. Approver routing writes nothing new and reaches only reviewers; `share_work` is never copied (ADR-011).
6. The refusal codes disclose only the state of a record the committer may read, after the member fence.
7. Model output is data: every member is validated twice (Python, Rust), and no primitive adds a URL, a path, code or a grant.

## 15. Risks

- **Fairness bound.** A close and an answer that cross inside one organization sequence step are ordered by the lock; the loser is refused. The plan tells Victor a late answer is refused, not silently dropped.
- **Current record = most recently touched.** Closing an old item after posting a new one makes the old one current for the countdown and the picker's default. Class guidance covers it; a "current pointer" is post-class.
- **The jsonb cast per row.** Bounded to one creator's rows; measured in APP-203's campaign at 25 × 50.
- **Calendar.** Five builders, two Tier A reviews, one edition, nine days, with release B's merge in the middle. The fallback is release B's features (formative quizzes), and Release C is not a go/no-go gate for the codes.
- **Composer reliability.** One example and one prompt pass; the real-model smoke is run once with the acceptance sentence and recorded; Victor builds on Nov 25 with the helper account to catch misses.

## 16. Consequences

APP-202 opens the edition; APP-203 to APP-207 implement; APP-208 is the stretch under its cut rule; APP-209 promotes. The plan's D11 is re-decided (its amendment names this ADR). Phase 2 of the leaderboard is named, not designed. ADR-002 gets an edition note from APP-202; ADR-007 is unchanged.

## 17. Clarifications, 2026-11-14 (APP-202, builder B0)

Taken while opening edition `impl-2026-11-14.2`, each the narrowest reading that keeps one encoding per meaning. Each is enforced by both validators and has a negative vector; `contracts/README.md` ("Edition impl-2026-11-14.2") lists the admitted shapes.

1. **`via` and `now` live in their own definition.** §11's "`guard.via`" is published as `$defs.requirement`: the APP-107 guard plus an optional `via`, whose `value` is a typed value or `$defs.now`. `$defs.guard` itself is unchanged, so a transition's or an initial rule's `when`, a countdown's `closed_when` and a query filter refuse `via` and `now` at the schema step (Rust: an unknown field or an unregistered literal type), with no semantic rule to keep in step. Storage (B1) may still model a `requires` entry as `rules::Guard` with `via`, as §6 says.
2. **An explicit zero offset is refused.** §§4 and 6 give `offset_seconds` the range −604800..604800 with "absence means 0"; §2 refuses a second encoding of a meaning. Both members admit −604800..−1 and 1..604800, and absence is the only spelling of 0 (as `visibility: "shared"` and `reviewer_only: false` are refused in edition `impl-2026-11-14.1`). The composer omits the member for a zero offset.
3. **A record filter may not target a countdown.** §3 defines the current record over every record of the input the viewer may read; a filter would give the countdown a second meaning ("current among the filtered rows"). A `record_filters` target naming a countdown is refused with the existing `record filter target scope mismatch`.
4. **A ranking's `group_field` may be `multi_enum`.** §9's semantics (a row counts once per selected value) need it; the rule's message becomes `record ranking requires a text, enum or multi_enum field`. Every other refusal of §9 stands.
5. **Order of the `multi_enum` checks.** A guard or typed filter literal is typed first (`compatible`: a subset in declaration order), then refused for its `multi_enum` field. An out-of-order or unknown selection therefore reports `guard value type/unit mismatch` (or `filter value type/unit mismatch`), an ordered one `guard field cannot be multi_enum` (or `filter field cannot be multi_enum`). In a definition both are refusals; the order rule governs stored values, which `validate_bundle` and storage judge with the same `compatible`.
6. **The two 422 codes are 422 only.** `validate_problem` refuses `requirement_not_met` or `unchanged` at any other status (`requirement_not_met and unchanged are 422`); the existing pairings keep the recovery view, the current revision, a limit and Retry-After off them. Rust admission gains `admit_problem`, the counterpart of `validate_problem`, so that §11's problem-body vector has a Rust verdict; it judges a nested recovery view by its member set, kind, version and reason only (storage builds it from typed values).
7. **The Release C specimen is not a bundle.** `contracts/fixtures/participation.json` holds §10's example at schema versions 2 and 1, a multi-select definition, the publication naming `countdown`, a record-components receipt naming a countdown and the two 422 bodies, each judged on its own. `validate_bundle` needs a query receipt naming a query view, which §10's example has none of, and its outcome identity rule describes a create that adds its requested record; a replacing create's outcome (the matched record at n+1, §5) is not modelled by the synthetic bundle check and is qualified by APP-203's campaign.
8. **Two mechanical lists outside §13's B0 row.** Every file a Rust source embeds is bound by `app/src/storage/managed_qualification.rs` `SOURCES` and admitted by `app/qualification/Dockerfile.dockerignore`, and the seed pins `contracts/validate.py` (`SEED_INPUTS`). B0 added its eight fixture lines there and re-pinned the validator; `contracts/lock.py` (the edition and the retained identities) and the new `contracts/fixtures/participation.json` are B0's too. Any later builder that adds a Rust source file (B1's `participation_campaign.rs`) adds its line to both lists in its own branch; the lists are sorted, so the rebase is mechanical.
