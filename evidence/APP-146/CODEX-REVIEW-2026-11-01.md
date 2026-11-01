# App-model spike source review — 2026-11-01

Read PR 22 at `f2926bf55710237e6dee3f632505c9889a5c6dfb` against trunk `c5b9452`. This is a documentation/source audit, not runtime qualification or acceptance of the spikes. No product code changed.

## Corrections

1. `app.md` previously compressed “an app” into one definition and work while naming only `app_id` and `work_id`. It now distinguishes the authorization/authoring container, publication and saved work. `choose_authoring` in `app/src/storage/composer.rs` reuses an owned non-library container; `draft_publish_save` and `save` in `app/src/composer/routes.rs` publish and create a work. Migration 0002 explicitly binds saved work to its publication and binds records and sources to that work. A share or page must identify the exact work under the trusted organization, not use only app id or the container's latest publication.
2. `APP-146` described ownership as holding `edit_definition`. Corrected it to `genui.apps.owner_principal_id`, matching the actual app note and schema. The separate edit/run checks still govern authoring.
3. `widget.md` and APP-148 said all widgets had no app id and implied no governed backing. Notes actually uses a saved work of the Notes library app, exposes app/work ids through `/v1/notes`, and reads/writes audited records (`app/src/actions/routes.rs`, APP-109 module notes). A folder's `BoxRow` can have an app association; `create_box` checks Run for an app-associated box (`app/src/storage/boxes.rs`). The updated note keeps the product widget category without denying these storage/authority contracts. It also distinguishes the existing folder Ask route from the absent general widget scope.
4. `app.md` said APP-145 was not built. It now records implementation on open PR 25 without claiming merge or acceptance.

## Product-model gaps to keep explicit

Persistent names and aliases (`app_labels`, `member_labels` in `app/src/storage/labels.rs`) are keyed by app id, while successive generated product apps may reuse one app id. Independent naming of those saved works is not established. Before claiming it, test generating A then B under one authoring shell, renaming B, and reopening/sharing A. A must retain its own publication, records and intended label.

Sharing defaults to read-only (`parse_share` in `app/src/sharing/mod.rs`). The page's share body containing only `grantee_principal_id` therefore does not make the recipient an editor. A trial promising both users can edit must explicitly request the existing bounded `record_write` option and test the resulting grant, or describe the recipient as a viewer.

For APP-139, bind a pending share to organization + exact app/work + verified recipient and only grant it after successful admission under current sharer authority. Invitation is not ownership. Retain denial/revocation/replay checks, plus recovery after interrupted signup. Test that another work in the same authoring container and unrelated source data stay unavailable. Existing source-read shares are organization-scoped; this spike does not change that permission model.

The demo also needs a fresh recipient browser, retained exact-work navigation after signup, actual source ingestion if files appear in the recording, and a live update observed without manual refresh in both relevant surfaces. Existing APP-144 acceptance promises page/board visibility only after refresh, which does not establish the user's instant-update request.

## Checks

- `python3 Tickets/_tools/board.py render`: passed, 152 tickets, 8 phases, 14 sprints, 80 requirements.
- `python3 Tickets/_tools/board.py check`: passed, including design digests and active links.
- `git diff --check`: passed.

No browser, service, model, database, cloud or remote mutation was run. Ticket status remains Backlog. No new acceptance box was checked.
