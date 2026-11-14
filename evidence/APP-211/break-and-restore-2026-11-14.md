# APP-211: withheld scores and grading by name, proofs and checks (builder B7, 2026-11-14)

Branch `release/c-b7-labels`, built on `72ce086b` (`release/c-participation`, the integration of B0–B5). The decisions are [ADR-020](../../docs/architecture/ADR-020-PARTICIPATION-PRIMITIVES.md) §17.34 (fix A) and §17.35 (fix B), taken by the coordinator in lieu of Victor after the class guide's review of Release C's record components. The commits are named in the ticket log.

## What changed, in one paragraph

**A.** A `record_summary` or `record_ranking` that scores against a key list (`matches`) is now **withheld** from a viewer who cannot read that key: the key's collection is declared `own` and the viewer is neither the app's owner nor a reviewer of the work. The decision is made once, in the capture (`storage::record_components::capture`), with the same SQL clause `own_rows` applies to every row the viewer did not create (`authorization::hidden`), so no second definition of a reviewer exists and every reader of the run agrees. A withheld component never reaches the engine, so no zero is computed; the result document and the receipt carry nothing for it; every page and the JSON batch show the state `withheld` and exactly "Scores are shown to this app's reviewers.", with no number, rows, groups, filters or receipt. The owner's and the reviewer's pages are unchanged, as are `count`, `sum`, every `matches` whose key is readable, the engine, the receipt kind, every digest domain and the wire shape. **B.** The record chooser of an update or transition form labels each record of an `own` collection `<creator label> · <text> · revision n`, using the name the Records card's "Filed by" already shows; shared collections keep today's label byte for byte.

## The interface

| Piece | File | What it does |
| --- | --- | --- |
| Capture | `app/src/storage/record_components.rs` | Binds every named input once; for each `matches` key input asks `SELECT $collection = ANY(hidden(app, work, SCOPED))`; partitions the components (`partition_withheld`); captures only the inputs the computed components name; `RecordComponentsRun::withheld()` lists the withheld declarations in definition order. Under `authorization::BYPASS` nothing is withheld. |
| Props | `app/src/query/record_components.rs` | `WITHHELD_NOTE`; `withheld_props(run, parameters)`; `props` and `unavailable_with_parameters` append them after the computed ones. `parse_result` and `receipt` are unchanged: the engine's components must match the payload's one to one, so a result naming a withheld component is refused. |
| Route | `app/src/query/record_routes.rs` | `compute(engine, run, parameters)`, the one function the work page, the canvas and the JSON refresh go through (`execute` delegates to it): a run with nothing left to compute still answers the note. `released` is `pub(crate)` for the campaign. |
| Chooser | `app/src/query/routes.rs` | `record_choice_label(creator, values, version)`; `document_forms` passes `directory::label(organization, creator)` when `detail.own`. |
| Storage detail | `app/src/storage/commands.rs` | `Published.own` from the schema's `visibility`; `RecordDetail.own`. |

No contract, edition, migration, SQL schema or client change. The record-components HTTP response keeps its shape: `state` takes the value `withheld` and `message` carries the note.

## Tests first, and what the first runs found

The unit tests and the campaign checks were written with the code and run before any proof. Two of the first runs were red for reasons in the tests, one for a reason in the code:

- `query::routes::tests::an_own_collections_chooser_names_the_creator_and_a_shared_one_does_not` first failed on an integer-valued record: the chooser's existing rule takes the first *string* value, and integers are stored as strings (`"4"`), so the label was `4 · revision 1`, not `Record · revision 1`. The expectation was wrong; the shared label is kept byte for byte, so the test now uses a boolean.
- The `record-components` campaign's new check first asserted that a contributor's capture still lists their own responses. It does not: the only components reading `responses` are the three withheld ones, so that input is no longer captured either, and the contributor's snapshot lists `items` alone. The check now says so.
- The campaign's D7 check then failed **under the bypass** with `Unavailable`: with the row rule bypassed `hidden` is the literal `ARRAY[]::uuid[]`, so the statement named none of its parameters and PostgreSQL could not type them. `key_hidden` now returns `false` under the bypass before any statement, as `own_collections` does; nothing is withheld, and the check fails under the bypass as every D7 check must.

## The record-components campaign (APP-211 section)

`just campaign record-components` keeps APP-204's participation app (a shared `items` list, an `own` `responses` list, an `own` `expected` key list filed by the owner; Ben Okafor, Ana Lima and a member with no display name as contributors; a reviewer by a reviewer share). The checks added or changed:

- **APP-211 (§17.34)**, for each of the three contributors: the run withholds exactly `right`, `leaderboard`, `top` (the `matches` summary and both rankings), in definition order; the snapshot lists `items` only (neither the key nor the responses, whose only readers are withheld); the engine's result has no outcome for them while the countdown and the item list compute; the receipt admits in Rust, validates under `contracts/validate.py` and covers none of them; the page props (`projections::props`) and the canvas batch (`record_routes::compute` then `released`, serialized as the client receives it) are equal and show, for each, `state: withheld`, the note, `has_summary/has_groups/has_receipt: false`, an empty summary and no groups.
- **APP-204 C4** (owner and reviewer): unchanged ranking (`Ben Okafor 2, Ana Lima 1, Member c0000c05 1`), and `withheld()` is empty.
- **APP-211 (§17.35)**: the chooser labels (built as `document_forms` builds them, from `read_record_detail` and `list_records`) over the `own` responses name Ana Lima, Ben Okafor and Member c0000c05 for the owner and the reviewer (five entries, three parts each); Ben sees two entries under his own name and the third member one under the neutral handle; the shared items read `Which colour? · revision 2`, `Which shape? · revision 2` (newest commit first) for everyone.
- **APP-211 D7** (runs as shipped and under `BYPASS`): the contributor's three components are withheld with the note while the reviewer's leaderboard is complete; under the bypass nothing is withheld and the check fails.
- **APP-204 D7** (reworded): a contributor's capture reads neither the key nor the responses and its creators map names the owner alone (from the items); a reviewer's reads all five responses and both key rows.

The `participation`, `private-entries` and `composer` campaigns were run unchanged except for the composer's acceptance 3, where the contributor's page now withholds the leaderboard and still counts their response.

## Break and restore

(Filled in below after the run.)
