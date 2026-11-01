# APP-141 creation-flow review — 2026-11-01

Reviewed PR 26 at `ed4e49683efa0384e171e662fdc193880c1f69ed` against trunk `c5b9452`. Corrections below are local changes in `pivot/app-141-create`; this record does not claim a merge or deployment.

## Findings and disposition

| Finding | Evidence | Disposition |
| --- | --- | --- |
| Create can open existing work instead of creating it | `openCreateApp` passed only the sentence to `submitIntent`; `compose_instant` routes an unpinned intent through the matcher, where retrieve and near-tie are legitimate outcomes | Fixed: confirmation sends `{generate: true}` through the existing compose route. No new grants or bypass of admission. The intent bar's normal matcher behavior stays intact. |
| Favorites cross members on the same browser origin | One global `cv-template-favorites` local-storage key contradicted the member preference contract | Fixed: scope by the existing canvas member digest, reject missing/invalid member keys, and catch storage failures. Do not adopt the unowned old key. |
| The UI describes prompt examples as templates | The four entries contain only handwritten starter sentences; selecting one transfers no admitted definition or template identity | Corrected product copy to “example description.” Actual admitted-template selection and instantiation remain unfinished APP-141 acceptance. Generated results keep the server's true provenance. |
| Admin regression suite fails on the branch | Before corrections, `npm --prefix app/admin test`: 55 passed, 1 failed. APP-124's upload test still expected its removed Describe click handler | Updated that assertion to reflect ownership by the creation module; added a real modal-handler/HTTP test for selection, cancel, confirm and refusal. |

A native `<dialog>` is already recognized by the canvas keyboard guard. No keyboard-guard change was needed for this modal.

## Verification

- `npm --prefix app/admin test`: 58 passed, 0 failed. New checks cover selecting and cancelling without a request; Create sending the edited sentence with `generate: true`; a refused response fetching no canvas group; member separation; ignoring the former unscoped favorites; and blocked storage.
- `npm --prefix app/admin run typecheck`: passed.
- `npm --prefix app/admin run build`: passed in strict native SSR mode. Canvas has one registered island; canvas JavaScript gzip is 161,003 of 373,760 bytes. No generated output was edited.
- `python3 Tickets/_tools/board.py render` and `check`: passed, 148 tickets, 8 phases, 14 sprints, 80 requirements.
- `git diff --check`: passed.
- `just check` was attempted before the admin build. Its Rust checks encountered the existing missing-`app/admin/dist` ordering problem. The run was stopped while the parent audit took ownership of the full integrated checks and that script-order fix. It is not recorded as a pass here.

No new browser or real-model run was performed in this branch. The parent audit owns the integrated browser check. Ports 4001–4010 were not touched. No AWS action, remote push, merge, or GitHub comment was made.

## Integration and remaining work

APP-145 supplies the authoring shell for a real new member. This branch alone cannot prove that path: the clean dev stand pre-creates a shell, while an invited empty member does not have one. APP-145 and APP-141 must be tested together without that dev preparation.

APP-142 can present the result as one united page, but this branch's `submitIntent` still uses the existing board placement path. App-page integration should establish the intended presentation explicitly. APP-144 still owns the standalone HTML app view. APP-139 still owns sharing to someone without an account.

The server intentionally returns neutral compose refusals, so the modal must not promise to display internal causes such as `no_authoring_app` verbatim. Likewise, the four example prompts are not a finished industry catalog and do not satisfy template provenance or instantiation requirements. Ticket acceptance stays open.
