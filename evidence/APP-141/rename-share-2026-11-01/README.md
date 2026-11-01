# Local rename through live share check

The final browser run passed on the owned 7601 member / 7604 account-only guest stand. The existing guest grant and connected placement were already active and both guest pages were open before the saved-work rename. The owner used the normal Library name editor.

After the real rename HTTP 200, the recipient page title was observed at 2,105 ms and connected canvas title at 1,096 ms (15-second assertion deadline; measured observations, not claimed exact propagation time). Neither page navigated or reloaded. Share ID, status, version, expiry and add permission were unchanged. Work publication and work version remained identical. An independent sibling under the same authoring app retained its title and publication/work pin. The connected card kept its x/y/width. Guest workspace access remained 404 and member commands/Ask/work links were absent. No page errors. Before/after screenshots were inspected.

Command: `node .cache/reports/rename-share/browser.mjs`

Evidence: `report.json`, `recipient-before.png`, `recipient-after.png`, `connected-before.png`, `connected-after.png` and the script. Synthetic local identity only; this does not qualify hosted signup. Prior exploratory attempts and the corrected header-selector issue are disclosed in the report. No source/tracked files were edited. The owned stand was restarted with optional guest 7604 enabled, preserving the same database volume.

Retained after PR 31 merged. Source identity in `report.json` is the local product baseline `0ec5e94`; candidate `711d87d` subsequently changed only a qualification fixture and runtime package pins. All four screenshots were independently inspected before retention. The script preserves the original local Playwright import/output paths and requires its disposable fixture JSON plus an accepted test share; it is an audit reproduction artifact, not a one-command hosted test. The only address literal in the script is the synthetic `example.test` fixture. Original artifact hashes are in `retention.json`.
