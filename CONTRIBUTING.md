# Contributing

This repository prepares the Forma company app platform from its delivered design. The current milestone is an organized workspace and implementation backlog. **No application runtime is claimed to exist yet.** Implementation work is Backlog or explicitly Blocked on an upstream gate; the first future implementation starts by selecting a ready ticket, initially APP-001.

Read [Tickets](Tickets/README.md), the [source guide](docs/planning/SOURCE-GUIDE.md) and [existing work](docs/planning/EXISTING-WORK.md) before choosing work. The platform's architecture is typed `AppDefinition` interpreted by trusted first-party renderers, immutable Parquet/DuckDB source revisions, and PostgreSQL for work, records, publication and audit. Northstar and Spend Review prove the same platform, not separate product codebases.

## Work in small, complete changes

Use the local `Tickets/` Markdown board as the source of truth. Choose one ticket with satisfied dependencies, assign its owner, record the branch/work context and keep its acceptance evidence with the change. Follow [implementation](Tickets/_tools/IMPLEMENT.md) and [review](Tickets/_tools/REVIEW.md). Categories describe responsibility; sprints describe staged outcomes. Neither numeric order nor a sprint label forces unrelated work to wait when dependencies permit parallel progress.

Inspect the current checkout and preserve other work. Reuse a suitable worktree or isolate a conflicting change as needed. Review donor components individually against current contracts; importing an older application wholesale is not a supported shortcut. The **GenUI team** owns application implementation. The **FormaStack team** owns upstream blockers and coordinates framework/fAuth delivery through the existing FORMA tickets. Keep both teams moving in parallel where possible, and make each blocked integration explicit with its owner and required evidence. Do not turn all upstream work into a blanket stop for the app.

## Source and design discipline

Frozen files under `design/` preserve the delivered source packet. Do not edit them to make implementation appear compliant. Record new implementation decisions and deviations in maintained planning/decision documents, cite the affected design requirement and link the owning ticket. Keep source provenance and historical evidence separate from current acceptance.

Use the requested published Forma 3.2 baseline when the foundation ticket creates the application. Pin the exact selected package set and commit the resulting lockfiles. An optional package or future feature needs its own verified version; old design assumptions are not proof it shipped. Use the forma-stack skill for the framework contract and the target repository's actual scripts once they exist.

## Verify what changed

The available preparation checks are:

```sh
python3 Tickets/_tools/board.py render
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

These validate the board, local links and frozen design hashes; they do not build or test an application. The foundation work will establish runtime build, test and development commands. Until then, do not invent successful app checks.

For implementation, run checks appropriate to the changed behavior and all ticket-required gates. Add focused regression coverage for defects and meaningful acceptance checks for new behavior. Record failures and limitations honestly. Independent review must assess the actual change before completion. Keep secrets, credentials, personal data, dependency folders and disposable build artifacts out of commits; retain sanitized evidence needed to understand results.

Use concise commits and review descriptions that explain the problem, resulting behavior and verification. Do not add fabricated coauthor attribution. Scope external actions to the current user request; preparing tickets alone does not perform or establish authority for a production rollout.
