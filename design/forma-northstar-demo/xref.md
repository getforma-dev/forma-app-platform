# Reference cross-check: forma-northstar-demo

The finished document checked against the research it was built from, by one fresh agent per
slice (`reference/PROCESS.md` section 6, gate item M26): the guide bullet by bullet, every number
recomputed, the pattern catalogue, the reader's edition and the notes against the record, the
teaching layer and its reading list. The reports are in `output/xref/`; this ledger is the lead's
triage of every finding in them. `npm run xref -- forma-northstar-demo --ledger` adds a `pending` row for
each finding not yet here.

Record: commit 9b9379f with the gate fixes, checked on 2026-10-23. Slices: guide, numbers, patterns, edition, teaching.

## Findings

| Finding | Severity | Disposition | Where or why |
|---|---|---|---|
| G-1 | should-fix | fixed | diagram 02 draws P1, the host's edge ending TLS, between the recording browser and the workspace server; 5.2's left-out clause dropped (with P-1) |
| G-2 | should-fix | fixed | the parser is a WebAssembly module in the worker with no system interface in 5.2, 6.4, 10.2, 10.5, 12.2 and TH10; diagram 02 draws it inside the host's network |
| G-3 | should-fix | fixed | diagrams 01 to 04 use the platform's boundary series, the compiler printing each id once; diagram 01's canary grouping is B10, named in 5.1, and section 7 lists B9; 6.6 cites P1 for the edge |
| G-4 | should-fix | fixed | section 8 states the level per path: READ COMMITTED with the named locks for commands and adoption, SERIALIZABLE for publication, the database enforcing each |
| G-5 | should-fix | accepted | platform 7.3 holds message for message for act 6's command and is cited; the record adds diagram 04, the operator's flows, which the platform lacks, and the body is over its page budget already |
| G-6 | should-fix | fixed | diagram 01's fAuth node reads "Hosted sign-in, the platform a registered client; EdDSA tokens verified offline"; K4 is ruled in 3.2 and the intake |
| G-7 | should-fix | accepted | 35 portrait body pages and 4 landscape diagram pages against 30 after the gate's additions (the operator's flows, the security review's rows and controls), the length pass having kept every fact; the body is inside the L2 word ceilings' tolerance (M17), which is the calibration the platform's GR-1 raises; the head decides at the summary |
| G-8 | should-fix | fixed | the summary says three operator tools, the reset under a rehearsal flag |
| G-9 | minor | fixed | 10.4's lead gives the rating scale and the rule; appendix C's intro says its rows are the ones a platform control covers, re-rated for one process and one operator |
| G-10 | minor | fixed | diagram 02's labels start with a verb; diagram 01's operator arrow names HTTPS to the API and the platform key, the provider arrow a verb; F12 carries the operator's profile |
| G-11 | minor | fixed | P9 and P10 close with a cost sentence (6.7) |
| G-12 | minor | accepted | section 4 is at its ceiling; 4.1 already names the canary, the script and the recorder as callers (and section 7 their flows), and N3 gives p50 and p99 per flow, the tail being what a retake is measured on (11.3) |
| G-13 | minor | fixed | section 9's idempotency paragraph: retrieve by key, then re-submit only when no outcome exists; a key in flight, a missing key; 7.2 step 4 says the same path |
| G-14 | minor | fixed | the threat rows are appendix C; every citation follows |
| G-15 | minor | fixed | 11.4 carries the parser-dies row; 11's lead says one process (D1); 11.1 ends the take at 10.6's shredding, which is the decommissioning; the burst alert is accepted away: one actor at a time cannot reach it (5.2) and the dashboard carries memory and CPU |
| G-16 | minor | fixed | 12.2 D1 option (B) names the shared-database antipattern |
| GR-1 | minor | reference | reference/GUIDE.md section 3: a dependent record names its parent and pin, may point at the parent for a card's contract, the component and deployment diagrams and the rating scale, and keeps its own context, container, fit check, threat rows, failure modes and cost |
| GR-2 | minor | reference | reference/GUIDE.md section 4: levels are not mixed within one diagram unless its title says so |
| N-1 | blocking | fixed | the expansion fixture is a 4 MiB XLSX that inflates past 50 MiB (7.1, TH10); A6's caps are a tenth of the platform's A4 (6.4, 6.8) |
| N-2 | should-fix | fixed | 7.1 act 3: the synthetic rows, eight here and twenty at most; TH11 and 13.1 follow |
| N-3 | should-fix | fixed | 4.2 defines ten rehearsals of up to two takes, twenty takes; N2's maximum is over twenty; one restart a take (11.3, 13.2) |
| N-4 | should-fix | fixed | 11.2 prices in US East (N. Virginia) at 730 hours and says a European region adds about a tenth |
| N-5 | minor | fixed | the small-items line recomputed from the claims check: snapshots free to the provisioned 20 GiB, egress inside the free 100 GB, three IPv4 addresses; $24, the total about $170 |
| N-6 | minor | fixed | "not three" dropped; 11.2 derives 120 attempts from two each for acts 3 and 6 a take over twenty takes, plus the spike |
| N-7 | minor | fixed | one epoch pair across 6.1, 6.5 and 7.1 |
| N-8 | minor | fixed | 4.2 states the serial basis: one actor at a time at one action per 5 s, about 360 interactions; the four-session envelope is the burst case (5.2) |
| P-1 | should-fix | fixed | with G-1 and G-3 |
| P-2 | should-fix | fixed | 7.2 step 3 and section 8: the unique index on (organization, work, record, version) holds version 0, since FOR UPDATE on an absent row locks nothing; FOR UPDATE holds versions 1 and up |
| P-3 | should-fix | fixed | 6.7 and SI7: role reset sets one organization per transaction under the same row-level security, no BYPASSRLS, owns no table, asserted per build (M2); section 8: FORCE ROW LEVEL SECURITY under a NOLOGIN owner |
| P-4 | should-fix | fixed | 5.2 and 11.1: the supervisor restarts the binary in place inside the health check's grace, so N2's 30 s triggers no task replacement; the drill confirms the task id unchanged |
| P-5 | should-fix | fixed | 6.4: a 1 s connect and 60 s request timeout; provider_unavailable within 1 s is the connect failure |
| P-6 | should-fix | fixed | 6.7 and 9: the reset is 409 while any job of either organization is claimed; TH8's row carries the race |
| P-7 | minor | fixed | section 9 argues 409 over 422 for a payload mismatch, one error class per scene, and adds the in-flight and missing-key answers |
| P-8 | minor | fixed | 11.2: the provider key in its own account unit with a spend limit; the limit's error is non-retryable, surfaced as provider_unavailable |
| P-9 | minor | fixed | 11.1: the per-build test signs in through both adapters, so the switch to the minimal login is a configuration change to a proven path, before a take |
| P-10 | minor | fixed | 11.3's dashboard carries the task's memory and CPU against M19's headroom |
| E-1 | should-fix | fixed | appendix A defines a take as one recording of an act or of the whole run; 4.2, 5.2, 11.2 and 11.3 name every whole-run count; the edition and the notes follow |
| E-2 | should-fix | fixed | a new chapter 9 gives the partner security and operations in brief and the accepted risks, AR1, AR3 and AR5 in plain words, before the engineer's chapter |
| E-3 | should-fix | fixed | notes Q21: a lost host loses nothing committed and the rehearsal restarts; only a lost database falls back to the snapshot (6.9, 8, 11.4) |
| E-4 | should-fix | fixed | chapter 8 states only what the record holds: five steps (platform 11.1), steps 1 to 3 true at birth, the platform's one-way decisions cited, not listed |
| E-5 | should-fix | fixed | the notes name each diagram by number and section with the current build's pages (11, 13, 21, 24) and its 45 pages; GUIDE 7's rule (ER-1) |
| E-6 | should-fix | fixed | Q8 asks why not a no-code tool, answered from 12.2 with no product claims |
| E-7 | should-fix | fixed | the intake's per-section profile met (4 as a ranked list in chapter 7, 10 and 11 as paragraphs in chapter 9, 12 in chapter 8); the intake's shape line updated to the revised edition |
| E-8 | minor | fixed | about $170 a month, hosting and tokens together, in the edition and the notes |
| E-9 | minor | fixed | 7.1 act 3: the field is data that can still steer the proposal, admission holding (TH11); the edition and Q22 use the same words |
| E-10 | minor | fixed | every act and chapters 9 and 10 open with their question and answer |
| E-11 | minor | fixed | admission, index, vocabulary, publication, evidence recorder and expected version defined at first use; RDS, S3 and KMS out of the partner's part |
| E-12 | minor | fixed | long sentences split, one idea each; the partner's part averages about 21 words a sentence |
| E-13 | minor | fixed | 7.2 and the edition's act 6 end "That is the end"; acts 3 to 5 carry a "What can go wrong" lead-in |
| E-14 | minor | fixed | generality and isolation in bold in the summary; chapter 7 says "so we measure" |
| E-15 | minor | accepted | a landscape diagram takes a page of its own, so the page before it can end short; the text before diagrams 02 to 04 was lengthened and act 6 tightened, and the edition is 15 portrait pages, inside its budget |
| E-16 | minor | fixed | the thirty-second pitch is 76 words |
| E-17 | minor | fixed | RK4's last cell: P9 rerun, the states recaptured from s0 (11.4) |
| ER-1 | minor | reference | reference/GUIDE.md section 7 and template/notes.md: the walkthrough names a diagram by number and section, a page number only from the current build's npm run pages, re-read before delivery (7743125) |
| T-1 | blocking | fixed | the D2 and D3 rows restored to 5.3 from 9b9379f, where the phase 7 consistency pass had dropped them; the 5.3 note's paragraph explaining their absence deleted |
| T-2 | should-fix | fixed | the 3.2 note: co-editing is a demo reduction (6.3, 6.8), the visible conflict is act 6's |
| T-3 | should-fix | fixed | the appendix C note's stale link warning deleted (the record now prefixes platform M16) |
| T-4 | should-fix | fixed | the D4 note: fAuth supplies a real tenant boundary in a signed token, not a second wall |
| T-5 | should-fix | fixed | the glossary's scene matches appendix A: one of the six acts, or a filmed moment within one |
| T-6 | minor | fixed | note 1: four families continue the platform's numbers, TL, SI, AS and TA |
| T-7 | minor | fixed | the 10.4 note: branch 3's fixture state, TH2, is the demo's own control |
| T-8 | minor | fixed | the builder defines a mitigation introduced in prose (M29), factory |
| T-9 | minor | fixed | the builder strips inline markup without a stray space, factory |
| T-10 | minor | fixed | the SR4 note and the Object Lock term say compliance mode |
| T-11 | minor | fixed | the exercise says 13.2's current choice, not a ruling |
| TR-1 | should-fix | accepted | deferred to the next reference pass at the head's request to deliver (2026-10-23); GUIDE section 3 already holds the dependent-record reading (8403503), and this record's annotations apply it by hand |
| TR-2 | minor | accepted | deferred to the next reference pass at the head's request to deliver; the method layer's history paragraph is generic |
| TR-3 | minor | accepted | deferred to the next reference pass at the head's request to deliver; wording between GUIDE section 1 and TEACHING, no effect on this design |
| TR-4 | minor | accepted | deferred to the next reference pass at the head's request to deliver; an example label in TEACHING |
| TR-5 | minor | accepted | deferred to the next reference pass at the head's request to deliver; this record states its own trusted base (10.2) |
| TR-6 | minor | accepted | deferred to the next reference pass at the head's request to deliver; this record's threat rows name their assets |
| TR-7 | minor | accepted | deferred to the next reference pass at the head's request to deliver; a list in TEACHING |

**Disposition** is what the lead did, and the gate holds the ledger to it:

- `fixed`: the record, an edition, the notes, a diagram or the annotations changed; say where.
- `reference`: the finding is about the reference, and the reference is corrected in its own
  `reference:` commit; name the file. Findings numbered with an R after the slice's letter
  (GR-1, PR-1, TR-1) and every S finding are about the reference.
- `accepted`: true, and left as it is; say why. A blocking finding is accepted only by the head,
  and the reason names them.
- `disputed`: the finding is wrong; say why, with the evidence.
- `pending`: not decided yet; the gate fails until none is left.

## Notes

The three slices agreed on two defects the reviews also found: the load balancer missing from diagram 02 (G-1, P-1) and the parser sandbox described two ways (G-2); both were decided once and applied everywhere. The numbers slice forced the one blocking fix (N-1, a fixture the record's own cap would refuse) and the pricing of the small items moved with the vendor-claims check (N-5). Two findings are the reference's: GUIDE now has a reading for a dependent record (GR-1) and its diagram-level rule no longer contradicts itself (GR-2). Accepted: the page count (G-7), the missing sequence diagram of the record's own (G-5), and two guide lines that section 4 and 11 have no room for (G-12, part of G-15).
