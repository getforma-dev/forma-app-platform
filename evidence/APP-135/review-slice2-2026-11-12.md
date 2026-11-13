# APP-135 slice 2: independent review (2026-11-12)

Scope: commit `44fded0` on `app135/clean-audit-2`, the "Register (2026-11-12, slice 2)" section and its log line in `Tickets/2-in-progress/APP-135.md`. I did not write it. I read the code only: no browser, no AWS, no dev stand. Line numbers are at `44fded0`.

Check: `python3 Tickets/_tools/board.py check`, which printed `OK: 178 tickets; 9 phases; 15 sprints; 80 requirements; design digests and active links verified.` (exit 0).

## Blocker

None.

## Major

**M1. "A file uploaded while the parser was missing stays unreadable, and nothing re-parses it" does not match the code.** No parse result is stored anywhere:

- The upload stores the object and the file row, then answers `preview: {"refused": "parser_unavailable"}` (`app/src/boxes/routes.rs:556-560`, `:446-451`).
- Folder Ask parses every candidate from the immutable original on each request: `app/src/boxes/ask.rs:491-505` calls `routes::inspect_file` (`routes.rs:423-444`), which calls `inspect::parse` (`inspect.rs:136`).
- The preview route also parses on each request (`routes.rs:821`).
- Dev state survives a restart (`GENUI_DEV_STATE`, `app/src/dev/mod.rs:204`, `:505`).

So after `just parser` and a restart, the next Ask should read the old file. The only thing that does not refresh is the canvas's one-time upload status, "… was stored, but it could not be read as a table" (`app/admin/src/canvas/boxes.ts:453`, `:457`). Correct the sentence. If the walk really did see this, record the request (folder, file id, response): the code does not explain it, so it would be a bug.

**M2. The folder Ask row misses a silent partial answer.** When at least one table can be read:

- Files that cannot be read are skipped, and their count is thrown away. `unreadable` is used only when no table was read (`ask.rs:496-505`, `:598`, `:614-623`).
- Files beyond `MAX_TABLES` = 3 (`app/src/boxes/mod.rs:48`, `ask.rs:499`) are never considered.
- Neither the model input nor the response (exactly `{"text","references"}`, `ask.rs:172-191`) mentions the skipped files.

A total over a folder with one unreadable CSV, or with a fourth CSV, therefore answers from part of the folder without saying so. Row truncation is disclosed (`partial`, `ask.rs:89-90`, `:200`, `:218-219`); skipped files are not. The case also arises in release, for files the parser refuses (for example, more than 32 columns, or formulas). This is what acceptance test 2 forbids ("No user path on the clean profile fails silently"). It needs a register row: fixed or ticketed.

## Minor

**m1. Naming is attributed to the wrong ticket.** The behaviour described is accurate. `name` builds a new saved work through `create_work`, using the source's publication and bindings and a context that starts with the marker and slug (`app/src/composer/commands.rs:422-451`). The response's `work_id` is that copy (`:477`). But the design is APP-101: the module doc (`commands.rs:1`, `:23-28`) and `Tickets/4-done/APP-101.md:90` ("creates a named copy (DS-55, no migration)"). APP-113 added only the per-member uniqueness constraint 0012 and the naming lock (`commands.rs:360-364`, `:455-457`; `Tickets/0-backlog/APP-113.md:180`). Change "(APP-113)" to "(APP-101; uniqueness APP-113)".

**m2. "It needs a model" holds only outside a demo organization.** A demo organization gets rehearsed answers with no model (`app/src/composer/talk.rs:95-97`, `:425-428`, `:839-840`); qualify both rows. A related miss on the Shape path: the no-model refusal shown in every organization suggests "only red invoices", "total by month" and "highlight overdue in red" (`app/admin/src/canvas/shape.ts:201`, used at `:218`). Those are rehearsed sentences that work only in the demo organization (`talk.rs:789-790`, `:815-822`). On the clean profile they get the same refusal. That is an APP-135 rehearsed-sentence hard-code. Card and app Ask use neutral wording (`app/admin/src/ask/client.ts:13`). The demo hint in `canvas/ask.ts:204` is unreachable: `:370` maps `composer_unavailable` to `folder_unavailable`.

**m3. The share row implies mail is sent somewhere else.** "No mail is sent on the stand" suggests other environments send mail. `app/src/` has no mail path at all (no SMTP, SES or mail client), so the sponsor has to deliver the `/s/…` link themselves. Say that. The rest checks out:

- The route is `/v1/work/{app}/{work}/shares` (`app/src/sharing/live.rs:174`).
- `create` (`:266-296`) answers 200 `{"share": …}` with `href: /s/{key}` (`:208`).
- `expires_in_days` defaults to 7 (`app/src/storage/shared_access.rs:41-43`, `:51-52`) and must be 1 to 30 (`live.rs:284`, `shared_access.rs:362`).

The register elides the request body. If it left out `expires_in_days`, write "default 7 days".

**m4. Ask on a single file gives a misleading message when the parser is missing.** With a `file_id`, the answer blames the file: "Ask could not analyze this file. Analysis currently supports simple CSV/XLSX tables with up to 32 columns …" (`ask.rs:616-620`), when the real cause is the missing parser. The log records only `no_tables` (`ask.rs:615`). The `parser_unavailable` code from `routes.rs:428` is dropped at `ask.rs:502`. In practice this happens only in development, because the image refuses to start without a valid parser (see below).

**m5. The folder row has no verdict.** The acceptance test allows four verdicts: works, fixed, demo-only or ticketed. "Accurate but not actionable" is none of them. Suggest "Works with the parser; dev-environment dependency, not ticketed", or ticket the message.

**m6. Bookkeeping.** No exact commands, ids or responses are kept as evidence (AGENTS.md: "record exact commands/results"). The log line names no checks. Branch / PR still lists only slice 1's branch.

## Verified as written

- **Parser log line.** The startup line in `app/src/dev/mod.rs:351-365` matches the quote. `just parser` is `Justfile:51-53`, which runs `scripts/dev/parser.sh`. The parser is resolved once, at startup (`dev/mod.rs:513`), so a restart is needed after building it, as the row says.
- **"Could not be read" message.** It is `no_table(1)`, `ask.rs:536-546`.
- **Row references.** They carry the file id, name and SHA-256 (`ask.rs:180-187`).
- **Release images carry the parser.**
  - `app/Dockerfile` has a parser stage (`:37-53`), copies the parser and its manifest (`:71-72`) and sets `GENUI_PARSER_MANIFEST_FILE` (`:85`).
  - At startup the server refuses to run if the manifest does not match (`app/src/main.rs:67-70`). `app/runtime/qualify_image.py:38` checks the same.
  - Caveat: if that variable is unset, the server leaves the parser off without logging it (`main.rs:258-262`; APP-087 nit n4 carries this). The image always sets it, so the claim holds.

## Verdict

Request changes; documentation only, no blocker. Fix the re-parse sentence (M1). Add a register row for the folder Ask partial answer (M2). Qualify the attribution, the model and mail wording, and the folder row's verdict (m1 to m5).
