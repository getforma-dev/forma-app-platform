# App assistant actions — local qualification, November 2

Scope: Victor's local-only follow-up on `pivot/local-ui-iteration`. No push, PR, merge, release, migration or database reset. Existing untracked notes and evidence are preserved. The persistent Northstar profile serves only port 4001.

## Behavior

- One App assistant launcher toggles the panel and resumes its selected tab; Ask, Summary, Sources and Saved remain inside it.
- Ask has explicit Ask / Change data modes. The latter selects an existing app action and, for updates, an exact record. Typed proposals stay unsaved until Confirm and save. Building/theming remain separate.
- Proposals bind app, work, publication, command, collection and record version; validate with the command's existing rollback dry run; confirmation uses the existing audited/idempotent route. Failed/stale/unauthorized proposals never fall back to app creation. Apps with no data actions remain read-only; source files are unchanged.
- View original opens the existing larger paginated viewer directly. Receipt JSON and metadata remain retained, while technical footers are hidden on app components including Focus. Partial/truncated result notices remain visible.

## Verification

Actual browser on port 4001 (real model, synthetic Notes fixture):

1. One assistant icon; toggling closes/reopens the selected Saved tab with correct expanded state.
2. Sources → View original opens the full modal directly, reads all five rows/five columns of the synthetic CSV, and leaves no inline viewer. Closing returns to the Sources tab.
3. App query/record receipt elements are hidden but retain their JSON/metadata. The app remains one frame across Card/Open and saves.
4. File-only app with no record commands gives a clear unsupported-action message.
5. New Notes work created by `/app:notes`; valid Add proposal produced displayed typed values with **zero records saved**. Cancel kept the count at zero.
6. Explicit Confirm created exactly one record; Update proposed only the requested body with old → new values, retained the title, and left version 1 unchanged until Confirm. Confirmation returned version 2, and Open showed the updated record.
7. Review is a separate visible step with keyboard focus; Cancel restores the untouched description. At 390×844 the assistant moves below the app and its review controls remain scrollable/usable. Cancelling that review left the record at version 2. The synthetic Notes placement was removed afterward; its test work/record remain saved.
8. First real-provider proposal revealed a complete Markdown JSON fence around valid output. The parser now accepts that one envelope only, while strict inner JSON admission and unknown-field rejection remain. A subsequent real-provider Add and Update both passed.

The only created browser data is synthetic work `854aad50-966a-4a24-a7b3-d91df870b6a3` and record `832ca330-43ff-42ff-b559-07ee71c34ee5`, titled “Local assistant verification”. Existing records/uploads were not edited. Logs remain in ignored `.cache/reports/`; screenshots stay under `/tmp/` rather than Git.

Independent review covered backend scope, authorization, typed units, rollback-only proposals and version/idempotency fences, plus frontend stale/hidden callbacks and uncertain-save retry. Findings repaired: proposal currency/decimal formatting, UUID collection validation, stale cached review visibility, legacy transition classification, and empty receipt table columns. Seven backend units and the isolated `just campaign actions` passed (36.39s), including no-write drafts, explicit create/update, unknown/cross-work/foreign/cross-site refusals, stale versions, duplicate replay, competing updates and grant revocation. The first broader native run found the required new module missing from the managed source bundle; it was added and its focused regression passed.

## Boundaries

The current record inventory returns the newest 200 authorized records. Truncated update choices are labeled; the assistant never chooses an unseen record. File viewing, read-only Ask and builder contracts retain their own bounds. Unsent drafts/conversation are session-local; saved answers keep existing durable behavior. Local sign-in is synthetic and does not test hosted provider sign-in.


## Final checks

- `just check admin`: passed, 255 tests, TypeScript and production build. Canvas gzip JavaScript 211,439 bytes against 373,760; Forma runtime 25,940 against 28,420 (see ignored build report for exact asset hashes).
- `just check fmt clippy-dev test http-smoke`: all passed. 393 Rust library tests, 8 binary SSR tests, 3 doctests; 34 opt-in campaigns ignored by the unit runner. Named actions campaign separately passed as above.
- `python3 Tickets/_tools/board.py render` and `check`; `git diff --check`: passed.
- Final loopback listener inventory: only 4001; 4002, 4003 and 4010 remain stopped. Preview HTTP 200. Original data and unrelated untracked notes/evidence remain intact.
