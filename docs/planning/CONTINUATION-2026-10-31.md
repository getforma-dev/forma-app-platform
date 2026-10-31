# October 31 continuation: implementation and remaining gates

Use [DEVELOPMENT](../DEVELOPMENT.md) for commands and repository rules, the
[module map](../architecture/MODULE-MAP.md) for ownership, and the authoritative
[ticket board](../../Tickets/TRACKER.md) for acceptance. This record preserves
current technical work after the approved relocation of session handoffs into
the local, gitignored `founder-notes/handoffs/` directory.

## Integrated work

- `pivot/pitch-mvp` is the trunk. `main` remains frozen for the APP-097 revisit;
  force-push and branch deletion protection remain in place.
- `pivot/review-routing`, `pivot/ladder`, and `pivot/pitch-script` were already
  merged when this continuation began. The pitch script was reviewed again:
  its fallback no longer pre-populates the clean customer's account, and its
  OpenAI claim now names the missing live smoke.
- `pivot/ci-green` is merged at `93df321`. All 12 workflows passed on its source
  branch at `89fd03a`. Integration moved pitch workflow commands to the root
  Cargo workspace and repaired missing Docker inputs (`actions/guard.rs` and
  `library/time-off.json`). Exact merged-source CI is recorded in APP-131.
- APP-135 contains the clean-slate audit register and targeted fixes. It remains
  an audit in progress; source inspection or deterministic campaigns must not
  be represented as a complete live clean-customer walkthrough.
  The live Haiku reading-list smoke passed after clarifying required empty
  arrays: one records input, two views, one create command, no transitions,
  no admission refusals. Generated and adapted origin now persists on reload;
  old definitions without recorded origin receive neutral copy.
- APP-136 owns the developer guide and corrected procedure pointers; APP-137
  owns the shared private beta ledger lookup across worktrees (ADR-013 step B).
- The [founder-notes audit](FOUNDER-NOTES-AUDIT-2026-10-31.md) records the approved
  movement of only the two session handoffs. Product/demo narrative, frozen
  design and authorization evidence remain tracked.

## Local and deployment boundaries

The pitch personas are on 4001–4003 and the existing clean-profile customer is
on 4010. Keep their data; announce restarts and never reset or stop another
owner's instance incidentally. Use `just dev status` and `just dev clean status`
to inspect the supervisors. `just restart` and `just restart clean` rebuild the
server while preserving data. Only reset a clean customer intentionally.

The latest **recorded** beta is TD:12 (`3901992`); see
[RELEASES](../pitch/RELEASES.md). No AWS deployment is authorized by this pickup.
The previous decision was “Not yet, I'll test.” Mutating deployment steps still
require the user's own words for that action through `--approved`. No claim of
fresh AWS observation is made here. The OVH getforma.dev site has separate
repositories and an independent delivery procedure.

## Remaining work, in order

1. Finish the breadth of APP-135: every clean-customer path (including sharing,
   MCP and Slack), with honest works/fixed/demo-only/ticketed verdicts. Prompt
   guidance for simple lists passed one real-model smoke; it is not a
   deterministic admission guarantee. Existing gaps include the dev-only
   Workspace link returning 404 without a hosted provider, and the legacy
   full-page work view showing source-query controls for records-only apps.
   Agree the keep/remove/defer list before deleting product features.
2. Rehearse APP-096 with the current [script](../pitch/PITCH-SCRIPT.md) and
   [recording checklist](../pitch/RECORDING.md). Preserve a genuinely empty
   customer for the new-customer beat; another tab of the same account is not
   an isolated fallback.
3. Review APP-129's seven contract gaps, APP-110's deferrals and the APP-097
   November 3 decisions. Keep Python for the current MVP unless that revisit
   explicitly changes the execution model; no Rust rewrite is part of pickup.
4. APP-099's real OpenAI end-to-end smoke still needs the appropriate key and
   actual execution. Adapter/unit coverage is not that smoke.
5. ADR-013 step H remains the next approved beta window: qualify the three
   images, perform each approved `just deploy` action, verify, and retain the
   window's `release.json`, release-history row and beta tag. Existing mock
   deployment tests do not satisfy this live gate. Production does not yet
   exist; its stack/account/budget decisions belong to APP-097.

No phase gate or historical acceptance is closed by this maintenance pass.

## Verification of this continuation

Local checks passed: 345 Rust library tests (30 intentionally ignored), six
binary SSR tests, three doctests, the
composer Docker campaign (generated and adapted provenance reread from the
database), formatting, clippy with and without `dev-stand`, Rust 1.85 MSRV,
campaign catalog, 48 admin tests, typecheck and strict SSR build, infrastructure
checks, Python runtime/scripts/operator/qualification suites, contracts/schema
pins, shell syntax and the regenerated board. The 12 operator tests use fake
AWS transports and temporary repositories. Independent review findings on
inherited paths and UI copy were fixed before committing.

Browser checks confirmed the reading-list starter suggestions, the retained
Book Club app's neutral provenance, its full-page “Saved work” label, and the
pitch review query's 192 claims with a verified receipt and no submission
draft. Both local servers were restarted with data kept; all four persona
health endpoints returned 200 afterwards. Exact pushed-commit workflow results
remain in GitHub Actions; local acceptance does not stand in for those runs.

Foundation CI on `07e3c6c` caught two stale binary SSR assertions after the
copy changes. They were corrected to the intended neutral labels and generic
creation commands. The local `just check` recipe now builds admin assets
before running library, binary SSR and documentation tests, matching the
Foundation check and closing that local coverage gap.
