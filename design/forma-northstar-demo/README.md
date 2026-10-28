# Forma Northstar demo preparation

This task prepares an MVP demo design for **Forma, the company app platform**. Northstar is a fictional company and the lead example. The demonstration also creates a compact Spend Review app, so the audience sees a company build different tools through the same platform.

The paired [platform v2 task](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/README.md) defines the industry-agnostic product. This task defines a smaller, buildable proof of it. There are two design tasks, not separate platform implementations for security and finance. The previous insurance demo and security investigation packet remain historical inputs.

## Read in this order

1. [PRODUCT-BRIEF.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/PRODUCT-BRIEF.md): the product claim, six acts and scope.
2. [SCENARIOS.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/SCENARIOS.md): reproducible examples and acceptance evidence.
3. [Shared contracts](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/SHARED-CONTRACTS.md): one proposed object, authority and lifecycle model for both designs.
4. [BUILD-HANDOFF.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/BUILD-HANDOFF.md): allowed simplifications, implementation boundaries and evidence gates.
5. [intake.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/intake.md) and [STATUS.md](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-northstar-demo/STATUS.md): assumptions and where the factory starts.

Both supplied product definitions are preserved in `input/context/`. The **App Platform** definition matches the user's latest request and governs conflicts: Forma is the launch product; internal forms and records are first-release capabilities; connected source access starts read-only; a second app is part of the generality proof. Source pages and current stack evidence are indexed in the [platform source record](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform-v2/SOURCES.md).

The proposed factory level is **L2, target, standard** for one team operating a reduced deployment. Its platform dependency remains an L3 concern. This is preparation for the design run. `design.md` remains the factory skeleton, and no application, production readiness or factory delivery gate is claimed complete.

The key test is straightforward: can a user create, publish and use a different supported app without a developer changing platform code for that example? Two prebuilt dashboards do not answer it.
