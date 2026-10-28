# Evidence and source precedence

Reviewed October 22, 2026. The user request outranks statements in the documents. Both supplied PDFs identify themselves as version 1.0 on the same date; semantic agreement with the user's request makes the **app-platform** PDF primary. Its “Confirmed” labels are product-source assertions, not approvals recorded by this preparation.

`review/source-manifest.json` identifies inspected local files by SHA-256 and records release/checkout pins. A working-file hash does not establish equality with a repository commit. This was a read-only source review; framework/application suites and registry-archive verification were not rerun.

## New product inputs

| ID | Source and pages | Supports |
|---|---|---|
| S01 | [App-platform definition](input/context/forma-app-platform-product-definition.pdf), pp.1–6 | Forma is the launch product; four layers, company-created apps, flexible roles, no predetermined vertical |
| S02 | Same, pp.7–10 | Company home/spaces, click/slash navigation, stable app identity and creation/transformation |
| S03 | Same, pp.11–14 | Connected data versus internal records, forms/create/update, authorization, publication and maintained saved work |
| S04 | Same, pp.15–18 | Managed browser offering, staged integrations, six-act demonstration including a different app |
| S05 | Same, pp.19–21,24 | Example-app range; two meaningfully different apps without app-specific platform changes; customer/market learning |
| S06 | [Earlier platform definition](input/context/forma-platform-product-definition.pdf), pp.1,4–6,19,22 | Earlier prescribed security-first launch, now superseded where conflicting |
| S07 | Same, pp.5,11,13 | Already allowed flexible roles, organization publication policy and writable internal notes/status/ownership; these are retained, not invented by the newer PDF |
| S08 | [Prior security preparation](../forma-security-investigation-workspace/README.md), [contracts](../forma-security-investigation-workspace/CONTRACTS.md), [validation](../forma-security-investigation-workspace/VALIDATION.md) | Northstar semantics, original investigation fixture and security boundaries; earlier vertical positioning/general mandatory reviewer rule do not govern this pair |

Original source paths are [app-platform PDF](/Users/victorvillacis/dev/Forma/Notes/forma-app-platform-product-definition.pdf) and [platform PDF](/Users/victorvillacis/dev/Forma/Notes/forma-platform-product-definition.pdf). Identical copies are preserved in both tasks. The original 17-page security definition remains at [Forma Notes](/Users/victorvillacis/dev/Forma/Notes/forma-security-product-definition.pdf).

Both new PDFs were read in full (25 and 23 pages respectively); central product-model, demo/completeness and earlier decision-table pages were also rendered and visually inspected. Text extraction and images are temporary working material, not substitute source documents.

## Release and historical designs

| ID | Source | Supports |
|---|---|---|
| S09 | [Stack v3 record](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-stack/deliverables/v3/md/design.md:24), [final receipt](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-stack/deliverables/v3/evidence/E88-AUTHENTICATED-FINAL.json:2) | Released scope and retained publication evidence |
| S10 | [As-built reconciliation](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/docs/release/V3-AS-BUILT.md:3), [soak receipt](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-stack/deliverables/v3/evidence/E99-post-release-soak-independent-review.md:12) | Final P17 61/61; separate RC.5 soak of 3,660 active seconds/15 rounds/zero faults; do not describe that as a fresh final-source soak |
| S11 | [Release-pinned security policy](https://github.com/getforma-dev/forma/blob/2006423b21fe0f8dd5a510bd09bfe3b8748327a5/SECURITY.md), [local policy](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/SECURITY.md:24) | FMIR trusts application-build input; arbitrary generated FMIR admission is not a 3.0 feature |
| S12 | [HTML runtime](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/packages/core/docs/HTML-RUNTIME.md:278), [core security](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/packages/core/SECURITY.md:139), [frame protocol](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/crates/forma-server/src/frame.rs:107) | Trust tiers/grants, host responsibilities and restricted unreviewed preview; no data-query bridge inferred |
| S13 | [Response contract](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/crates/forma-server/src/response.rs:8), [native subset](/Users/victorvillacis/dev/Forma/work/codex-release-lead/forma/packages/compiler/docs/subset.md:32) | Intact policy-bearing PageResponse and native compiler boundaries |
| S14 | [Historical platform v4 decisions](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform/deliverables/v4/md/design.md:499) | D13–D27 artifact, review, identity, tenancy, data and surfaces; target design, not implementation |
| S15 | [3.1 proposals](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform/deliverables/v4/md/design.md:2107), [old assistant](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-generative-app-platform/deliverables/v4/md/design.md:2184) | Proposed framework additions and state-only/no-rows assistant, requiring explicit reconciliation |
| S16 | [Insurance MVP v5](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-insurance-demo/deliverables/v5/md/design.md:341), [data model](/Users/victorvillacis/dev/design-doc-factory/tasks/forma-insurance-demo/deliverables/v5/md/design.md:803) | Not built; financial upload/discard and latest-mode choices cannot become universal app-platform policy |

Canonical release: `getforma-dev/forma` tag `v3.0.0` resolves to `2006423b21fe0f8dd5a510bd09bfe3b8748327a5`. The [release page](https://github.com/getforma-dev/forma/releases/tag/v3.0.0) and retained receipt establish the October 20 publication. Current checkout HEAD is recorded separately in the manifest; later release evidence/docs are not retroactively part of that tag.

The inspected current framework SECURITY.md, V3-AS-BUILT.md, compiler subset.md and core SECURITY.md differ from their release-tag blobs. The manifest records both digests. Frame, response and HTML-runtime files inspected here equal their release blobs. Attribute shipped behavior to the release pin and later reconciliation to its own inspected snapshot.

## Built-app donor evidence

| ID | Source | Supports |
|---|---|---|
| S17 | [Vector Studio package](/Users/victorvillacis/dev/FormaBuiltApps/forma-vector-studio/package.json:22), [Cargo](/Users/victorvillacis/dev/FormaBuiltApps/forma-vector-studio/Cargo.toml:53), [architecture](/Users/victorvillacis/dev/FormaBuiltApps/forma-vector-studio/docs/architecture/overview.md:26) | Pre-3.0 pins; authoring/retrieval prior art and missing browser/WebSocket end-user auth |
| S18 | [Generative UI package](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/package.json:19), [README](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/README.md:81), [local data](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/src/local_data.rs:91) | Local prototype, core 2.0.1, explicit limits; not a managed company platform |
| S19 | [Runtime package](/Users/victorvillacis/dev/FormaBuiltApps/forma-genui-runtime/package.json:44), [provenance](/Users/victorvillacis/dev/FormaBuiltApps/forma-genui-runtime/docs/PROVENANCE.md:1), [security](/Users/victorvillacis/dev/FormaBuiltApps/forma-genui-runtime/SECURITY.md:7) | Extracted canvas scope, 2.x peer, caller-owned trust; Shadow DOM is not a security boundary |
| S20 | [Strict plans](/Users/victorvillacis/dev/FormaBuiltApps/forma-genui-runtime/crates/forma-genui-runtime/src/plan.rs:34), [catalog](/Users/victorvillacis/dev/FormaBuiltApps/forma-genui-runtime/crates/forma-genui-runtime/src/catalog.rs:186), [bindings](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/src/server/mod.rs:3161) | Useful exact structural binding/validation patterns; authorization remains platform work |
| S21 | [XLSX deferred](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/docs/architecture/dataset-widget-binding.md:367), [retrieval status](/Users/victorvillacis/dev/FormaBuiltApps/forma-generative-ui/docs/architecture/retrieval-and-routing-status.md:74) | New parsing work and lexical/hash baseline, not proven broad semantic retrieval |
| S22 | [GateWASM](/Users/victorvillacis/dev/FormaBuiltApps/gatewasm/Cargo.toml:11), [Fieldwork](/Users/victorvillacis/dev/Forma/forma-fullstack-demo/README.md:3) | Candidate identity reference versus actual 3.0 SSR/island consumer; neither supplies finished platform tenancy/data services |

The previous review's [source manifest](../forma-security-investigation-workspace/review/source-manifest.json) retains its original paths. The new manifest resolves the user-requested folder renames without rewriting that historical record. Selected unchanged source hashes preserve the earlier observation; they do not establish compatibility or a passed suite.

## Primary external references

- [AWS Builders' Library: safe retries](https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/): retrying after an uncertain response can duplicate effects; command identity and retained outcomes inform C07. The exact transaction/expiry design remains Forma's proposal.
- [MCP architecture](https://modelcontextprotocol.io/docs/2026-07-28/learn/architecture) and [MCP Apps overview](https://modelcontextprotocol.io/extensions/apps/overview): protocol/UI capabilities do not supply Forma's business authorization or promise host portability. Integration remains staged and host-qualified.
- [OWASP prompt injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/): evidence may carry adversarial instructions; retrieval does not remove that concern. Server-enforced capabilities are a design recommendation, not immunity.
- [YC request for startups](https://www.ycombinator.com/rfs#a-cloud-for-small-software): the small-software cloud request discusses deployment, sharing, authentication and permissions. Alignment is our inference, not endorsement, acceptance or evidence of demand.

AWS and MCP architecture were checked during this update; MCP Apps, OWASP and YC were checked in the earlier investigation in this conversation. Reverify mutable vendor facts during the full design claims phase. Provider pricing, processing locations, compliance status and production compatibility were not certified here.

## Addendum, 2026-10-22 foundation review

| ID | Source | Supports |
|---|---|---|
| S23 | [Native composition experiment](review/spike-native-composition/README.md): `@getforma/create-app@3.0.0` minimal template, published `@getforma/core`, `compiler` and `build` 3.0.0, `forma-server` and `forma-ir` 3.0.0 from crates.io | A typed-definition canvas (summary, table, timeline, form from one declared `cards` array, one island) is admitted in strict SSR mode and renders with its data on the server; an enum comparison in a native condition is refused (BUILD-610); the route's Forma code measures 27,674 of 28,100 B |
| S24 | [Foundation review](review/foundation-review.md) | Input fidelity, verified stack facts at the tag, the representation decision record, ticket dispositions |

The canonical checkout's `main` moved from `753be2ae` (the audit's recorded HEAD) to `b4faca0a` on
2026-10-22 through PRs 149 to 151 (package metadata, packed-resolution tests, symlinked
`outputDir` refusal). None changes a contract cited above; the release pin is unchanged.
