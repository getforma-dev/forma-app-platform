# Foundation review: what it means for the demo

The lead's review of the inputs and the stack before the run lives in the platform task:
[`../../forma-generative-app-platform-v2/review/foundation-review.md`](../../forma-generative-app-platform-v2/review/foundation-review.md)
(2026-10-22). This page records only what changes for the demo.

## The scenarios hold

`SCENARIOS.md` was read against the controlling definition's six acts (§16) and its completeness
test (§18). S01 to S07 cover every act and every completeness clause: two apps through one
authoring path, second-person reuse, reopen with context, revocation, a failing draft beside a
published version, binding to the new user's data, and the second-organization canary. The fixture
arithmetic recomputes (1,140,000; 900,000; 114,000 per minute; 210,000 and 21,000; 64,000 and
45,000 cents). Nothing in the scenarios changes with the representation decision.

## What the representation ruling changes here

| Under the typed definition (recommended) | Under design A's markup |
|---|---|
| Acts 3 and 6 are composition proofs: retrieve, adapt and generate views, queries, record schemas and commands over the first-party library; the "meaningful change" is a new query node (transfer rate with an explicit denominator) and a persisted form, both structural | Acts 3 and 6 generate markup modules admitted through the sanitizer and mounted with grants; the preview uses the frame |
| The spike's first step is the native composition fixture (`../../forma-generative-app-platform-v2/review/spike-native-composition/`) extended to the real library and the real canvas route, then the full path admitted definition → authorized query → receipt → internal command → transactional outcome → reopen (`BUILD-HANDOFF.md`) | The spike's first step is FORMA-237 to 241's proofs with their 3.0.0 fallbacks, as design B v5 appendix F had it |
| The AI story of act 3 and the handoff of act 5 gain the assistant's parameter changes (C12): a request such as "focus on the unmatched destination" sets declared filters across the linked views, marked and undoable | H4 as designed in A v4 |
| Framework dependencies: FORMA-232 and FORMA-246, both assumed done | FORMA-237 to 242, each proven in the spike |

## Demo-specific cautions from the stack review

- The canvas route's Forma-code budget had 426 B of headroom in the fixture; the spike measures
  the real route before any scene is rehearsed, and the design assumes FORMA-232.
- Every value shown is computed server-side and arrives as declared props; kind dispatch is
  boolean flags from the handler (an enum comparison is refused natively, BUILD-610).
- Pages paginate: the inert head state is capped at 1 MiB; Spend Review's queue and Northstar's
  evidence table page on the server, and their exact totals are computed before pagination (C06,
  S02).
- Forms work before activation (native forms post to the command API), so act 6's decision
  survives a slow or failed script load; the island adds conflict and retry feedback.
- The pitch sentence for Forma Stack stays as the definition's §15 puts it: the stack makes apps
  structured, composable and precisely changeable; it does not make competitors incapable. The
  HTML runtime's tiers and grants are the path to custom widgets later, not a first-release claim.
