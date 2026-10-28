# Forma generative app platform v2 preparation

**Forma is the platform companies use to create, run, transform and share their own apps. Northstar demonstrates it.** This packet prepares the next platform design and its paired demo design; it is not a delivered system design or an implemented service.

The user requested this product iteration on October 22, 2026. The `v2` folder means the new product direction, not the old platform's already-delivered document version v2. Earlier platform deliverables v1–v4 remain in `../forma-generative-app-platform/`. The prior insurance demo and security-workspace preparation remain historical references.

## The two designs

| Task | Purpose | Proposed level |
|---|---|---|
| This platform task | Industry-agnostic managed company app platform: creation, data, forms/records, publication, access, saved work and operations | L3 target |
| [Northstar demo](../forma-northstar-demo/README.md) | Buildable, reduced deployment of the same product; rich Northstar investigation plus compact Spend Review app | L2 target |

Two designs do not mean two product codebases. The demo must use the platform's object model and authority boundaries. It may reduce scale and deployment; it cannot replace generic creation with app-specific screens and call that the platform.

## Read in this order

1. [PRODUCT-BRIEF.md](PRODUCT-BRIEF.md): the revised product and launch capability boundary.
2. [ALIGNMENT.md](ALIGNMENT.md): source precedence, historical decisions, released stack and prototype reuse.
3. [SHARED-CONTRACTS.md](SHARED-CONTRACTS.md): the proposed contracts binding platform and demo.
4. [VALIDATION.md](VALIDATION.md): requirements, planning envelope and evidence gates.
5. [intake.md](intake.md), then [STATUS.md](STATUS.md): what is known, assumed and next.
6. [SOURCES.md](SOURCES.md) and `review/`: source snapshots and preparation checks.

Both supplied PDFs are preserved in `input/context/`. The app-platform definition governs where the two differ because it matches the user's current direction. In particular, the older security-first launch prescription is not inherited. Internal records/forms are first-release capabilities; read-only applies to connected sources.

The demo's sixth act creates a different app through the same supported authoring path. Spend Review is a recommended compact example, not a second industry product or a commitment to launch into finance.

No blocking clarification is needed to prepare these tasks. Proposed architecture, operating numbers and provider choices remain recommendations for the factory run. `design.md` is the untouched factory skeleton; begin from `STATUS.md`, not from a presumed finished design.
