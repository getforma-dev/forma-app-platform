# APP-191 / APP-192: break-and-restore proofs (W4, 2026-11-14)

Branch `trial/w4-private-entries`, built on W1's final `d8e88489` (migration 0023). Every test below was first shown to fail with its rule removed, then to pass with the rule restored. Where a rule lives in SQL that `catalog-v24.json` pins, the catalog was regenerated for the broken SQL (`app/runtime/generate_schema_catalog.py`) and the manifest re-pinned, so the fixture started and the test judged the rule, not the pin (the lesson from W1's proofs). The tree was restored byte-identically each time (`git status --short` empty on `app/migrations`).

| Rule | Where | Break | Result with the break | Restored |
| --- | --- | --- | --- | --- |
| Grammar: `visibility: "own"`, `reviewer_only: true`; `"shared"` and `false` refused | `admission/validate.rs` | `git checkout` of the validator change | `admission::tests::private_entries_and_reviewer_only_commands_match_validate_py` panicked at the first admit (`"own"` refused) | passes; the Python suite (43 tests), `verify_vectors.mjs`, `lock.py check` and `make_library.py --check` pass |
| D8 gate in `compose` | `storage/commands.rs` | the `require(... !declaration.reviewer_only || reviewer)` line replaced by a no-op | `storage::commands::tests::reviewer_only_commands_are_refused_for_contributors` failed at the contributor's approve (Ok instead of Denied) | passes |
| One row rule on every record statement | `storage/tests.rs` scan | one statement's `own = own_rows(...)` replaced by `own = "TRUE"` | `every_record_statement_carries_own_rows` failed naming the statement | passes (14 statements carry the rule, 7 allowlisted, each matching once) |
| `own_rows`, `hidden`, `own_collections`, the D8 gate at run time | Rust, `authorization::BYPASS` | the campaign sets the flag and re-runs each enforcement check | every check marked `(fails under bypass)` returned false under the flag (17 checks); the shared `notes` listing stayed byte-equal | all 33 checks pass as shipped |
| Reviewer-grant rule in the definers: only a reviewer grants the reviewer role | `0024` SQL, `share_work` and `set_work_everyone` | both `IF p_role='reviewer' AND NOT v_review THEN RETURN; END IF;` lines removed; catalog regenerated for the broken SQL; manifest re-pinned (`check-schema-pins.py` agree) | the campaign failed at the direct definer check: `share_work(BEN, FAY, 'reviewer')` answered 5 rows instead of 0 (30 earlier API-level checks still passed, because `Permit::also(RecordReview)` stands in front of the definer in production, which is why the direct call under the workspace role was added) | restored, catalog regenerated, tree byte-identical |
| `owner_only` derived from the canonical schema | `0024` SQL, the generated column | `IS NOT DISTINCT FROM 'own'` changed to `'never'`; catalog regenerated; manifest re-pinned | the campaign failed at its first check: no `owner_only` row (0 instead of 1) | restored, catalog regenerated, tree byte-identical |

Share authority (`share_work` never copied; a reviewer or contributor without it shares nothing) is asserted as shipped through the API and through the direct definer calls (`share_work(REV, FAY, 'viewer')` and `set_work_everyone(REV, 'viewer')` answer no rows); it was not broken separately: the guard is 0008's unchanged clause.

## Commands and results

- `cargo test --lib -p forma-app-platform` (417 unit tests, incl. the three new ones) and `just check fmt admin clippy clippy-dev test http-smoke msrv catalog infra py-scripts py-runtime shared-broker py-app-scripts py-ops py-infra schema-pins contracts bash-n board py-qualification`: ok.
- `just check schema-qualifier`: ok on the merged tree (73 passed checks; the 0024 expectations are in `app/runtime/qualify_tenant_schema.py`).
- `just campaign private-entries`: ok, 34 checks (`.cache/reports/private-entries.json`), the last being the purge exception of 0024 (W3 R1): a guest account's `share_events` row survives deleting its `live_work_shares` row, while `linked_apps` still refuses the delete until the linked app is removed; the dropped key is `share_events_organization_id_share_id_fkey` (0017), noted in the 0024 header as a recorded exception to ADR-009's additive rule, and the qualifier asserts the exact foreign keys of both identity tables.
- Regression campaigns: `sharing`, `record-components`, `live-sharing`, `mcp`, `slack`, `pitch`, `actions`, `canvas`: ok. `query`: ok when run alone (90 s); its saturation check had read `failed` instead of `loading/busy` once while other campaigns and `just check` ran concurrently (load, not the change).
- `just check py-promotion`: fails on this branch and on W1's alike: `ops/promotion/schema-release.json` is release A's committed record, bound to the deployment-contract digest of HEAD (`app/migrations/**`, the runtime and the image contexts). Any branch that adds a migration fails `test_committed_release_is_bound_to_this_tree` until the release owner commits release B's record (22→24 once W1 and W4 are merged). Not written here: the release record belongs to W1/W3 and needs Victor's approval.

## Limits

- The bypass flag is test-only (`#[cfg(test)]`) and process-global; a campaign runs alone, so no other test reads it.
- Presentation (which commands a listing shows) and the D9 share grants are asserted as shipped only; the bypass disables enforcement, not presentation or sharing.
- A pinned answer is its maker's alone today (0015's `CHECK (kind = 'card' OR NOT everyone)` and `share_card`), so the `member_cards` clause that would hide an answer over hidden rows is defence in depth; the campaign asserts the outcome (no other member lists it).
- MCP and Slack reach the records through the same storage reads the campaign exercises directly (`list_records`, `search_records`, `list_record_target`, `commit_record`, `dry_run_command`); their own campaigns stay green.

## Release B preflight on beta (read-only; review m5)

Run as the operator before release B, on beta's live database, in a read-only transaction. It derives `owner_only` for every stored record schema exactly as migration 0024's generated column will (`genui.canonical_jsonb` is `convert_from(canonical_schema,'UTF8')::jsonb`), so a row that would make the migration fail shows up here first, and the expected result is `would_fail = 0`, `non_utf8 = 0`, and `own` equal to the number of schemas declared `visibility: "own"` (0 before W4's grammar is in use).

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '30s';
-- 1. Every stored record schema parses as UTF-8 JSON and derives; a row that
--    does not is listed (none expected). `pg_input_is_valid` never raises.
SELECT organization_id, app_id, revision_id, schema_id, schema_version,
       octet_length(canonical_schema) AS bytes,
       pg_input_is_valid(convert_from(canonical_schema,'UTF8'), 'jsonb') AS parses
FROM genui.definition_record_schemas
WHERE NOT pg_input_is_valid(convert_from(canonical_schema,'UTF8'), 'jsonb');
-- 2. The derivation over every row (the migration's expression), with the
--    byte classes the old `encode(...,'escape')` trick would have mishandled.
SELECT count(*) AS schemas,
       count(*) FILTER (WHERE (convert_from(canonical_schema,'UTF8')::jsonb->>'visibility') IS NOT DISTINCT FROM 'own') AS own,
       count(*) FILTER (WHERE convert_from(canonical_schema,'UTF8')::jsonb ? 'visibility') AS with_visibility,
       count(*) FILTER (WHERE octet_length(canonical_schema) <> length(convert_from(canonical_schema,'UTF8'))) AS non_ascii,
       count(*) FILTER (WHERE position('\\' in convert_from(canonical_schema,'UTF8')) > 0) AS with_backslash,
       0 AS non_utf8  -- convert_from raised on none (the statement completed)
FROM genui.definition_record_schemas;
-- 3. Per organization and app, which schemas are `own` (expected: none today).
SELECT organization_id, app_id, schema_id, schema_version
FROM genui.definition_record_schemas
WHERE (convert_from(canonical_schema,'UTF8')::jsonb->>'visibility') IS NOT DISTINCT FROM 'own'
ORDER BY 1,2,3,4;
ROLLBACK;
```

If statement 1 lists a row or statement 2 raises (`invalid byte sequence for encoding "UTF8"` or a JSON syntax error), release B must not run: that row would fail the 22→24 transaction. Record the three results in the release record.

## Fix round after the Tier A review (`review-w4-2026-11-14.md`), 2026-11-14

Decisions taken (the coordinator's, in lieu of Victor, where noted):

- **M1, where the fix lives:** 0024 is amended in place rather than a new 0025. m1, m2, m3 and m5 change the bodies of the 0024 definers and the `owner_only` derivation anyway, so one migration keeps release B at 22→24 and one catalog chain; 0024 has not shipped. W3's `share_events` exception stays in 0024 with its header note. The 0024 SHA therefore changes (reported in the hand-over). The new definer is `genui_private.revoke_work_share(org, actor, app, work, grantee)`, callable by the app's owner alone: a `share_work` holder's lineage is not followed (owner only, as the coordinator allowed). It deletes every `record_read`, `record_write` and `record_review` row the grantee holds on the work, marks the grantee's personal share records revoked and appends one `permission.revoked`; the organization-wide share, the app-level `discover`/`run` rows and the owner's own standing are untouched. Storage API: `Workspace::revoke_work_share(request, app, work, grantee) -> Result<Revoked { grantee, removed }>` (`Revoked::changed()`); `Denied` for a non-owner, the owner naming themselves, an unknown work or a malformed id. The resurrection sequence (reviewer revoked, then an organization-wide viewer share) is a bypass-judged campaign check.
- **f1 (W3's reviewer):** `share_work` with a lower role deletes the rows the new role lacks in the same transaction (reviewer→contributor drops `record_review`, →viewer also `record_write`), audited as `permission.revoked` before the grant; a sharer who could not have granted the stripped rows is refused before any write. `Shared` gains `removed: Vec<String>`; `Shared::changed()` counts removals.
- **m1, m2:** both definers judge the sharer's personal standing (owner, unrevoked creator, work grants): `record_read` is required (0008's rule restored), the organization-wide share counts for nothing copied, and `set_work_everyone` requires `discover` and `run` on the app, so the role's set is only ever what the sharer holds.
- **m3:** `set_work_everyone` takes `pg_advisory_xact_lock(hashtextextended('forma:work-everyone:'||org||':'||work,0))` before reading the live row; a two-connection race check shows the second caller waiting, then replacing the role, both with results.
- **n7:** a live reviewer organization-wide share is lowered or revoked only by a reviewer.
- **m5:** `owner_only` is `(genui.canonical_jsonb(canonical_schema)->>'visibility') IS NOT DISTINCT FROM 'own'`, where `genui.canonical_jsonb` is an IMMUTABLE SQL wrapper over `convert_from(…,'UTF8')::jsonb` executable by `genui_workspace` (PUBLIC revoked). Invalid UTF-8 or non-JSON bytes fail closed at write time; an escaped spelling decodes to `own` (and admission never stores one: `admission::tests::private_entries_and_reviewer_only_commands_match_validate_py` admits `"\u006fwn"` and shows the canonical bytes carry `"own"` literally). The campaign writes a non-ASCII schema (derives true), an escaped one (derives true) and an invalid-UTF-8 one (refused). The beta preflight is below.
- **m7 (composer):** the generation prompt teaches `visibility: "own"` and `reviewer_only: true` from plain words, with a general "Suggestion box" example, and never the explicit alternatives; two APP-193 rules are added: "which X" is an enum or short text (a `reference` field in generated content is refused by the planner, code `reference_field`, with a retry hint), and an owner-advanced shared item is a reviewer-only **update** with no transition. Unit tests cover the generated members, the refused alternatives on each shape, the reference refusal and the owner-advanced fixture; the composer campaign generates both apps through the scripted model and proves the owner can advance their own one-row item.
- **n2, n3, n6:** the application roles read only organization-wide `work_shares` rows (a RESTRICTIVE policy); the unused creator index is gone; the empty `impl Workspace {}` is gone. **n4, n5** are recorded in the ADR-007 amendment as accepted windows.

### Campaign after the fixes

`just campaign private-entries`: ok, **49 checks, 24 of them judged under bypass** (`grep -c "judged!("`), including the review's m4 additions: receipts over another member's row, the query release of a contributor's snapshot, the member and guest source-view sequences (a second app binds the S05 expenses source beside an own collection), both guest read sites through fixture-inserted live shares the product refuses to create, and a contributor's reviewer-only update (`grade`) of their own claim over the command path. The `notes` listing stays byte-equal in both modes.

### Break proofs of this round

Rust-layer guards (each shown failing with the guard removed, then passing restored):

| Guard | Break | Result |
| --- | --- | --- |
| Composer prompt guidance and example (m7) | the PRIVATE ENTRIES paragraph removed from the prompt | `composer::tests::private_entries_and_reviewer_only_commands_are_generated_from_plain_words` and `composer::prompt::tests::the_private_entries_example_and_its_guidance_are_in_the_prompt` failed; pass restored |
| Planner refusal of `reference` fields (APP-193) | `require_usable_fields` call removed from `plan_generated` | `composer::tests::which_x_is_an_enum_or_text_and_an_owner_advances_their_own_item_by_an_update` failed (a reference admitted); pass restored |
| Every new bypass-judged check (m4, M1 resurrection, f1) | the campaign's `BYPASS` flag, in the same run | each returned false under the flag (24 checks) |

SQL-layer guards, each with `catalog-v24.json` regenerated for the broken SQL and the manifest re-pinned (`check-schema-pins.py` agree) so the fixture judged the rule, not the pin; the tree restored byte-identically afterwards:

| Guard (0024) | Break | Campaign result with the break (checks passed before it; the first failure) |
| --- | --- | --- |
| m1: the sharer must hold `record_read` (`share_work`, `set_work_everyone`) | both `IF NOT v_read THEN RETURN` lines removed | failed after 32 checks: "m1: a sharer without record_read confers nothing, not even record_read" (Kim's direct `share_work(…,'viewer')` answered rows) |
| m2: standing held only through the organization-wide share is not copied | the live org-share disjunct re-added to `v_write` in both definers | failed after 32 checks: "m2: write held only through the organization-wide share is not copied" |
| m3: concurrent `set_work_everyone` calls serialize | the `pg_advisory_xact_lock` removed | failed after 34 checks: the second concurrent call answered `Err("db error")` (the partial unique index) instead of a result |
| M1: only the owner revokes a personal share | `IF NOT v_owner THEN RETURN` removed from `revoke_work_share` | failed after 35 checks: "only the owner revokes" (Ben's revoke of Rev succeeded) |
| f1: a lower role strips `record_review` | the `record_review` demotion step removed | failed after 37 checks: Rev kept a `record_review` row after the contributor re-share |
| n7: a live reviewer organization-wide share is lowered or revoked by a reviewer only | the guard removed | failed after 33 checks: "n7: a non-reviewer cannot lower the owner's reviewer share" |
| m5: `owner_only` derives from the parsed schema | `IS NOT DISTINCT FROM 'own'` → `'never'` in the generated column | failed at the first check: no `owner_only` row |

Each break's catalog was generated by `app/runtime/generate_schema_catalog.py` for the broken SQL, the manifest re-pinned (`scripts/check-schema-pins.py` agree), the campaign run with `just campaign private-entries`, and the migration, catalog and manifest restored byte-identically (`cmp` against the kept copies) before the next break. Script: the session's `breaks.py`; results: `breaks.log` (quoted above).
