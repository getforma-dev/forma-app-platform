# Source guide and precedence

This repository prepares implementation from the October 23 delivered designs, with the user's October 28 instruction to use released Forma 3.2. It does not reopen the representation decision or adopt an old prototype as the application.

Read in this order:

1. [Platform delivered design](../../design/forma-generative-app-platform-v2/deliverables/v1/md/design.md): architecture, R1–R16, N1–N12, SR1–SR13, operations and threats.
2. [Northstar delivered design](../../design/forma-northstar-demo/deliverables/v1/md/design.md): reduced demonstration deployment and six acts.
3. [Shared contracts C01–C16](../../design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md), [scenarios S01–S07](../../design/forma-northstar-demo/SCENARIOS.md), and [contract lock](../../design/forma-northstar-demo/platform-contract-lock.json).
4. [Product brief](../../design/forma-generative-app-platform-v2/PRODUCT-BRIEF.md), [validation](../../design/forma-generative-app-platform-v2/VALIDATION.md), [intake rulings](../../design/forma-generative-app-platform-v2/intake.md), and recorded reviews for context.
5. [Implementation decisions](../architecture/DECISIONS.md) and [3.2 baseline](../architecture/FRAMEWORK-BASELINE.md) for current reconciliations.

The source README, readiness record, opening STATUS sections and BUILD-HANDOFF include older preparation language. They remain intact as historical evidence. They do not override the later delivered D1 typed definition, D4 storage choice or demo D2 managed-store choice. The contract lock's preparation-status label is historical; its digest remains useful and is checked. Resolve any substantive later conflict explicitly in APP-001.

The platform source identifies delivery commit `e279953` and tag `task/forma-generative-app-platform-v2/v1`. The [snapshot manifest](../../design/snapshot-manifest.json) records the actual copied working-tree bytes and source repository HEAD; it does not claim every byte was freshly rebuilt from that delivery commit. Both complete task packets were copied except factory `output/` drafts/intermediates and `.DS_Store`. Source files were not moved or edited.

The paired task folder names are retained so their relative links continue to work. Historical absolute links and links to other factory projects are archival citations, not portable implementation dependencies. All current planning documents use repository-relative links. The copied source includes PDFs and editable diagrams; use its `deliverables/v1/` editions for finished reading.

Published stack behavior supersedes the design's old v3.0 pin only where explicitly reconciled. Old byte budgets, 3.0 test results, pricing and hosting assumptions are historical measurements or targets, not newly verified promises. Existing upstream tickets retain their status until their owner closes them with evidence.
