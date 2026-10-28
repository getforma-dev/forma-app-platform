# APP-003 published canvas consumer qualification

This isolated synthetic fixture prepares the GenUI consumer gate. It does not close FORMA-246, publish FORMA-232, integrate the production app, or complete APP-003 by itself. The parent ticket remains Blocked until its upstream evidence and independent review are accepted.

The native route includes all nine launch primitives from design §6.5: summary, table, detail, timeline, chart, filter, form, note and status. A single registered Canvas island receives declared presentation props, boolean dispatch flags, stable list keys and bounded numeric SVG geometry from Rust. Six views expose the same filter scope/epoch. The local linked endpoint supplies fixed synthetic server results; the browser does no business aggregation. The form endpoint validates a small synthetic request and returns an acknowledgement without persistence, authorization, receipts or idempotent storage. Its fixed hidden key demonstrates native form transport only, not the production key lifecycle.

## Reproduce

Use Node 22.21.0, npm 11.6.2, Rust/Cargo 1.93.1 and Python 3. Install the three browser engines once:

```sh
npm ci --prefix spikes/APP-003/admin
cd spikes/APP-003/admin
npx --no-install playwright install chromium firefox webkit
```

From the repository root:

```sh
python3 spikes/APP-003/verify.py
```

The harness installs from committed locks, typechecks, builds strict SSR, enforces both the 28,100-byte app target and published 28,420-byte Forma cap, runs four compiler refusal controls, three Rust controls, release rendering and sequential Chromium/Firefox/WebKit checks. Rust compilation is bounded to two jobs; browser engines and pages run one at a time. The server binds loopback on an OS-assigned port, reports the owned address and is terminated/reaped by the harness. Generated output stays in ignored `admin/dist`/`target`/`.cache` folders. CI runs the same command with pinned tools and Playwright 1.64.1.

## Released package boundary

Core/compiler/build and forma-ir/derive/server are exactly registry 3.2.0, with npm integrity and Cargo checksums in their lockfiles. No local package archive, framework checkout runtime, budget override, router or Browser fallback is used. The starting manifests follow the completed APP-001 derivative of the published minimal scaffold. The source pattern follows the preserved design composition spike and the independently replayed FORMA-246 fixture; this is a new nine-primitive consumer fixture, not an unchanged replay of either.

The published TypeScript declaration calls JSX values `HTMLElement | DocumentFragment` while the positional list callback accepts only `HTMLElement | string | number`; it also omits the SVG `title` tag. `src/forma-types.d.ts` is a disclosed consumer declaration bridge for JSX callbacks and SVG title attributes. It emits no JavaScript and changes no installed package, compiler or IR. Every actual list row here has one Element root; native compilation and real-browser namespace/node checks qualify those concrete uses. This does not assert that arbitrary fragments are valid native list roots. The original typecheck errors are recorded as an implementation observation in the evidence README, not hidden with `ts-ignore` or disabled checking.

The compiler refuses a reassigned plain local derived from props (`BUILD-610`), and a negative unary `tabIndex` literal (`BUILD-207`, also seen through a module constant). The accepted source uses a prop-seeded signal and a server-provided numeric tab index. Enum comparison dispatch and four-level composition remain explicit refusal controls. No emitted IR or build report is repaired.

## What the evidence means

The browser blocks only the activation module until it has captured the native root and every descendant, then waits on the module's actual top-level activation promise. It observes zero initial mutations, original-node identity, and a seeded positive observer control. All five states (normal, no cards, empty nested arrays, long/non-ASCII/hostile literal labels, and exactly 500 table rows) must pass every engine. The normal state additionally tests real keyboard form submission, a no-JavaScript native form POST, out-of-order filter responses, accessible labels/live output/focused region, numeric SVG attributes and namespaces, and no script execution/page errors/CSP violations. The `pending` marker is never used as readiness. Accessible-state snapshots are not a human screen-reader qualification.

The release benchmark times full `PageHandle.render` calls, including a fresh RenderCtx/nonce and response construction; it does not time only the native walker. The returned `counts.bytes` is separately labeled native-walk HTML bytes, not the full document. HTTP evidence records complete-document and actual encoded island-envelope UTF-8 sizes. One first call per state and 200 sequential subsequent calls are local samples; later states run after earlier states and are not OS/cache-cold. Process lifetime high-water RSS comes from `getrusage`, so it is neither allocation per render nor incremental per-state memory. The host is macOS ARM64, not the planned 2-vCPU/8-GiB task. There is no N3 client/network/database p99 or deployment-capacity claim.

The over-cap UTF-8 state must produce `RENDER-502`, never truncate or return a Browser shell. Invalid geometry, excess rows, duplicate card keys, mismatched declared Rust props, unbound native routes and missing/Browser route modes are refusal controls.

Full canvas geometry/layout persistence belongs to APP-027, production renderer integration to APP-028, comprehensive accessibility/human screen-reader acceptance to APP-029, and durable record/filter-query/security semantics to their owning tickets. This spike neither implements nor claims them.
