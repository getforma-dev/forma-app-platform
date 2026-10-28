# Fresh review: the Northstar demo record (phase 7, L2)

Reviewer: a fresh senior engineer with no prior knowledge of the project, reading as the person who would build it. Inputs: `tasks/forma-northstar-demo/design.md`, the rendered pages `output/pages/page-01..39.png` (diagram pages 10, 12 and 20 inspected at page size), `reference/CHECKLIST.md` (R items), `reference/GUIDE.md` section 3, and the platform record `tasks/forma-generative-app-platform-v2/design.md` sections 4 to 13 and appendix D, read as far as the demo's citations reach. No briefs, STATUS files or input folder were read.

Date: 2026-10-22.

## 1. Counts

| Severity | Count |
|---|---|
| Blocking | 3 |
| Major | 10 |
| Minor | 21 |

Verdict: not ready at L2 as it stands; one focused fix pass away (section 5).

## 2. R items

| ID | Verdict | Evidence |
|---|---|---|
| R1 | partial | 5.3 carries a cost per decision; 12.2's records give reason and cost together. The three added components are uneven: P11 has its cost line ("a fifth role and a test on every build"), P9 and P10 have a ceiling but no cost line naming which of the four costs apply (F15). |
| R2 | partial | Most numbers derive (0.8/s, 4.8 ops/s, 24, 1,200 receipts, 60 KiB, the cost lines, 426 B, 165 ms). Four do not hold up: the burst's "at most 8 at the data service" rests on a mechanism the cited platform section does not describe (F4); the task's 2 vCPU / 8 GiB has no derivation against the memory the platform's components claim (F12); 4.2's "360 interactions" uses one session's rate while the fit check uses four (F14); the provider line's "six" attempts a take is unexplained (F23). Latencies are percentiles at the client (N3, 11.3). |
| R3 | pass | Every category has a number or "not required, because" (N1, N11); N10 is "the meter", in the platform's pattern. Ranked N9, N7, N6, N5, N2; section 1's optimisation sentence names generality and isolation, which map to N9 and N7. Minor: section 1 omits generation latency, which section 5's sentence includes (F26). |
| R4 | pass | N4 and SI7, SI8 are tagged safety or liveness; every write path names its isolation level and enforcer (6.2 SERIALIZABLE, 6.3 and 8 READ COMMITTED with explicit locks, the unique index on the key). |
| R5 | partial | Titles state type, scope and arrow semantics; legends and boundaries present; narration before and after; zooms name their parent. Fails on legibility: arrow labels and box descriptions on diagrams 02 and 03 are about 4 to 5 pt at page size (F11). Boundary id B2 means two things across diagrams 01 and 03 (F20). |
| R6 | pass | Every row is a full statement in the AWS grammar, cites elements and an invariant, separates existing control from mitigation, carries a response and a residual with the head as owner. Minor: the "reappears as" mapping of platform rows is wrong for TH21 to TH8 (F18). |
| R7 | partial | Alternatives name what they optimise for and what they lose against; 12.1 is separate; D1, D4, D7 are in decision-record shape with confirmation lines; do nothing and reuse or buy present and ordered by reuse risk. One alternative is vacuous: "the full component catalogue" (D5), because the platform's library is exactly the nine kinds the demo keeps (F5). |
| R8 | pass | 13.2 is bucketed before, during, later, each with candidates and the current choice or deferral. Minor: one entry is not a question (F31). |
| R9 | pass | No pasted schemas; nothing called fast or secure without a number or mechanism. |
| R10 | pass | "Unknown yet" (N10), RTO unknown, "Still unknown: the spike's three numbers, the task replacement, RTO, a month's cost". |
| R11 | pass | Five sentences; optimisation trade-off; scope; the one-way doors D2, D3, D7 with the event that closes D7, pointer to 5.3; matches the body. |
| R12 | pass | Pattern names follow their problem (memento in 6.7, adapter via the platform); seams stand in for extension points and each names the change it anticipates (growing into the platform, G7). |
| R13 | pass | Worked scenario (the six acts), fit check after diagram 02, key-decisions table, "like X, unlike X", and 11.2's explicit deferral of 10x. Weak: the positioning is against "a staged product demo", not a system the reviewer knows (F30). |
| R14 | partial | Flows are numbered phases with "That is the end" then "Things go wrong when"; worked examples with concrete values throughout; prose first. 3.1's first paragraph bolds the goals without their mechanisms (F30). |
| R15 | partial | Sections answer their questions and little exists for show. But a builder would not know what to do at four points: which fixture state a take of act 4 starts from (F1), whether the island re-POSTs or GETs by key after the lost response (F6), how P10 obtains a header-free capture from Chrome (F7), and whether Spend Review is published under review or owner policy and on camera (F8). |
| R16 | fail | Decisions trace to requirement ids and the summary traces to the body. Three citations of the platform do not hold as written: A6's caps are not the platform's (F2), "platform SR1 to SR12 hold" is false for SR3, SR8 and SR9 (F3), and 7.2 routes the record-schema check through P3 against platform 5.2 and 7.3 (F9). Five of the seven top threat rows cite elements (P9, P11, EE2, DS2) that no demo diagram draws (F10). |
| R17 | fail | Body pages 1 to 35 less three landscape diagram pages is 32, above L2's 30. The core (sections 1 to 5) runs pages 2 to 14, 11 portrait pages against "about eight" (F13). |
| R18 | partial | Elements named, typed, described with technology; arrows unidirectional with protocol in brackets; each diagram says what it simplifies. Diagram 02's arrow labels are noun phrases without verbs (F33); legibility as under R5. |
| R19 | pass | Section 2 tells three shortcut mechanisms naming the step and the number; the scope sentence is in section 1 and 3.2. |
| R20 | partial | 13.3 answers four objections with pointers. The reader is surprised twice: Finance's owner-publication policy first appears in 7.2 step 2 (F8), and `ik-3` first appears in 7.2 step 5 (F16). |
| R21 | partial | Security invariants with enforcers (SI7, SI8 plus the platform's), entry-point table, data dictionary referenced from the DFD, fail-closed per control, operator access as JIT with breakglass and two-person explicitly accepted away (AR1), SR1 to SR6 in section 4 with verification. The claim that the platform's SR1 to SR12 hold is false (F3). |
| R22 | partial | Rollout is one digest, no waves, rollback the previous task definition; migration cites the platform's five steps with bridge states and names the pre-pilot teardown. No rollback-trigger metric and no upgrade-downgrade test are named; the demo's rationale (no waves) is stated, so accepted with a note. Deployment view: the platform's diagram 08 with five reductions undrawn; acceptable as "the reason there is none". |
| R23 | pass | The spike is proposed (D8, R11) with its five measurements; every card states a ceiling and what it does not do. |
| R24 | pass | 4.2 carries a fault scenario (provider outage), a change scenario (the second app) and the duplicate retry, each with its measurement, though compressed rather than labeled six-part. 13.1 ranks on 10.4's scale. |

## 3. Findings

Format: id, R item or GUIDE rule, location, severity, problem, fix.

### Blocking

**F1. The take procedure cannot record act 4 from the fixture states it names.** R15, GUIDE 11 (operations runnable by someone who is not you). 11.1 "A take" step 2; section 8 "Retention and the reset"; 6.7 P9; TH2 M4.
Section 8 places `s2` "after the import" and `s4` "after publication"; M4 says "`s4` publication 1 only". 11.1 says a take resets to "`s4` from act 4 on". Act 4 is the publication scene: Maya's refused self-publish and Sarah's approval. A take starting from `s4` already holds publication 1, so act 4's refusal and approval cannot be recorded, and no state exists after act 3 (five draft revisions, nothing published). 6.7 says P9 captures states "from a clean take at each act's start", which contradicts section 8's boundaries. A builder cannot script a retake of act 4, and acts 5 and 6 taken from `s4` must silently replay act 4's live work and save first.
Fix: define the states at the act starts they serve (a state after act 3, before publication, for act 4; a state with Maya's saved case for act 5; one with Finance's import for act 6, or say act 6 replays act 2's path), make section 8, 6.7, 11.1 and M4 agree, and say what a take of acts 5 and 6 replays before the camera runs.

**F2. A6's input caps are presented as the platform's caps; they are not.** R2, R16. 3.3 A6; 4.2 N1; 6.4; 7.1 act 2; TH10.
A6 says "CSV 10 MiB, 100,000 rows; XLSX 5 MiB compressed, 50 MiB expanded, 10,000 by 32 per sheet: the platform's caps (N1), kept so the oversize refusal is real". Platform A4 is CSV 100 MiB and 1 million records, XLSX 10 MiB compressed and 100 MiB expanded. The demo's values are a tenth and a half. 6.4 then calls the same caps a reduction ("[reduced: the platform's input limits]"), which contradicts A6, and 7.1 and TH10 build the bomb scene on "past 50 MiB" where the platform's scene is "past 100 MiB".
Fix: either keep the platform's caps and say so everywhere (then the bomb scene inflates past 100 MiB), or keep the reduced caps and rewrite A6 as "a tenth of the platform's A4, raised when measured", consistent with 6.4 and 6.8.

**F3. "Platform SR1 to SR12 hold (10.5)" is false for at least three of them.** R21, R16. Section 4 after SR6; 10.5; 10.7; AR1.
Platform SR3 requires a signed service token on every internal call; 10.5 says "the platform's service token (platform M16) is not minted" and a typed in-process principal replaces it. Platform SR9 requires breakglass sealed with an outside reader and two-person destructive changes; AR1 accepts neither exists. Platform SR8 (signed releases built from pinned lockfiles with an SBOM) is nowhere addressed for the demo build, though 10.1 says platform TH6 holds unchanged. A security requirement declared to hold in section 4 that section 10 then removes is a false statement in the section the gate reads first.
Fix: replace the one-liner with a short table or sentence per platform SR: holds unchanged, replaced by (the typed principal for SR3), accepted away (AR1 for SR9), or inherited through the release path (SR8, naming where the demo build's image is signed and its SBOM kept).

### Major

**F4. The burst arithmetic rests on a mechanism the cited platform section does not describe.** R2. 5.2 fit check; 6.3 ceiling; 11.2 "Overload cannot occur".
5.2 says "the burst runs in twelve waves, each island fanning two queries at a time: at most 8 operations at the data service, 2 running and 6 waiting, under the 20 that return 429 (platform 6.3)". Platform 6.5 says a filter change "sends one typed query a linked view", six at once, and platform 7.2 step 4 confirms "fans six queries". With the platform's island, a burst of 24 from one organization is 2 running and 22 waiting, over the cap of 20, so two operations return 429 and "Overload cannot occur" is wrong. The island-side throttle of two in flight is either the demo's change (then it is a reduction with a seam, and 6.5 must say so) or does not exist. It also matters who the four sessions are: if the canary is one of them, Northstar's burst is 18 and the cap holds without any throttle.
Fix: state the mechanism and its owner, or recount the burst by organization; make 5.2, 6.3 and 11.2 agree.

**F5. D5's "nine kinds" is not a reduction of the platform's catalogue.** R7, R12. 5.3 D5; 6.5; 6.8 row 4; 12.2 "The full component catalogue (D5)".
Platform 6.5 names the renderer library as "summary, table, detail, timeline, chart, filter, form, note and status": nine kinds. The demo keeps nine kinds "[reduced: the platform's catalogue]", lists "Two apps, nine kinds, two operators" as a reduction with a seam, and offers "the full component catalogue" as a rejected alternative. Nothing is cut; the alternative compares the design with itself; a builder looking for the kinds to leave out finds none.
Fix: either name the kinds the demo omits (if the platform's catalogue is wider than its 6.5 list, say where), or drop D5's "reduced" tag, the 6.8 row and the 12.2 row, and keep D5 as "the platform's nine kinds, registered before the digest, 426 B to fit them".

**F6. The retry after the lost response is described two ways.** R15, GUIDE 9. 7.2 step 4; section 9 last paragraph.
7.2 step 4: "the island resends `ik-1` unchanged, as after any timeout", and the outcome is found inside P4's transaction before the version check. Section 9: "After a timeout the island retrieves the outcome by key before submitting", that is a GET by key first. The S05 scene the partner sees differs: a second POST hitting the unique index, or a GET returning the stored outcome. The script, the capture and the packet's row depend on which.
Fix: pick one. If the island GETs by key first, the scene is "the GET returns version 1" and the POST-duplicate path is a test, not a scene; say so in 7.2 and 13.3's "What is simulated".

**F7. The evidence recorder's capture contradicts itself.** R2, R15. 6.7 P10; 7.2 step 9; TH6 M11; appendix B (Chrome HAR).
P10 "drops every header, and refuses a capture carrying a `Cookie`, `Set-Cookie` or `Authorization` header"; 7.2 says the network log leaves the profile "every header stripped" (F12). Chrome's HAR export carries request and response headers, including `Cookie` on every authenticated call, so every capture the browser produces is one P10 refuses, and no mechanism strips headers "before it leaves the profile".
Fix: name the mechanism: P10 reads the HAR and strips headers itself (then the refusal rule applies to what it files, not what it reads), or the capture comes from a proxy or an instrumented island rather than a HAR export. Keep M11 consistent with the choice.

**F8. Act 6's publication step is missing, and publication policy is per organization in some places and per space in others.** R20, R15, R16. Section 5 act 6; 4.1 R7; 7.2 step 2; 6.2; 6.7; RK4.
Section 5's act 6 and R7 go from "forms the queue" to "commits version 1" with no publish step; 7.2 step 2 says "Finance's policy is owner publication ... so Maya publishes and starts live work by its alias". 6.2 and 6.7 speak of "Northstar's independent-review policy" and "both organizations' policy rows", as if policy were per organization; Security and Finance are spaces of the one Northstar organization. The partner's first question after act 6, "so the second app skipped review?", is unanswered, and a builder does not know whether the policy table is keyed by organization or by space.
Fix: add the publish step to act 6 in section 5 and R7, say it runs under Finance's owner-publication policy and why that is the honest demonstration of R9's second kind, make 6.2, 6.7 and the pre-take assertion speak of policy per space, and add a 13.3 objection for it.

**F9. The record-schema check is routed through the control plane, against the platform it cites.** R16, G7. 7.2 step 3 ("checks record write and the pinned schema's transitions from P3 (F5)"); diagram 03 F5; 6.9.
Platform 7.3 step 2 has P2 read the pinned publication's record schema from DS1 (F44), and platform 5.2 says published apps serve from the workspace server's own reads so "the control plane is off that path"; the platform's dependency table has the control plane soft. The demo puts P3 on every record command. In one process it costs nothing, which is why 6.9's single "process" row hides it, but at migration step 4 the seam G7 promises would put a soft role on the hard command path.
Fix: have P2 read the schema from the publication row (F4 or a DS1 arrow), relabel diagram 03's F5 to admission and publication only, and keep 6.9's statement that the control plane is off the run path.

**F10. The demo's own attack surface has no data-flow diagram.** R16, R6, GUIDE 7 ("one DFD per key flow"). Section 7 intro; diagram 03; 10.2 entry table; 10.4 TH1, TH2, TH5, TH7, TH8.
The demo adds three code paths, P9, P10 and P11, and the operator's shell EE2; five of the seven top threat rows and the entry-point row for the reset cite P9, P11, EE2 or DS2, and the reset row cites "F3 to P11". Diagram 03 draws P10 only. The elements exist in section 7's prose, which satisfies the script's M8, but a reviewer cannot see the arrows the reset, the provisioning script and the fixture states cross, which are the crossings TH1, TH2, TH5 and TH8 are about.
Fix: one small DFD of the operator's flows (EE2 to P9 to fAuth and the administration group; P9 to DS2 for the states; EE2 to P11 through P1 and P2 to DS1 and DS2; P10's F12 and F13), with F-ids the threat rows then cite.

**F11. Diagram labels are illegible at page size.** R5, R18, GUIDE 4. Pages 12 and 20 (diagrams 02 and 03); page 10 to a lesser degree.
On the rendered pages the box titles read, but the arrow labels and the box descriptions are about 4 to 5 pt (roughly 1.5 mm tall on A4 landscape). On diagram 03 the F-labels, the protocol and classification brackets, and the boundary names B3 and B4 cannot be read without zooming; these are the labels section 10 traces to. The 5.3 and 12.2 tables also wrap their narrow headers into fragments ("Serv es", "Se cti on", "Los es agai nst", pages 14 and 33).
Fix: fewer words per arrow (verb, object, protocol), a larger minimum font in the compiler, or split diagram 03 into the command path and the return-and-capture path; widen or rename the narrow table columns.

**F12. The task size has no derivation.** R2. 11.2 cost table; 5.2 restart; TH10 M19.
"One task: 2 vCPU, 8 GiB" is asserted. The platform's components the demo runs in that task claim memory on their own account: DuckDB at 1 GiB `memory_limit` per instance with up to 8 operations in flight at the data service (platform 6.3), the parser sandbox at a 2 GiB cap (platform 6.4), a 2 GiB result cache (platform 8), plus the four roles' pools. TH10's M19 ("the module's memory cap ... below the task's memory") and N2's 30 s restart both depend on the number being right.
Fix: one sentence summing the budgets at the demo's concurrency (2 running operations, one sandbox, cache empty) and the headroom, or lower the in-flight caps for the demo and say so.

**F13. The body is over L2's page range and the core over its budget.** R17, GUIDE 6. Pages 1 to 35; sections 1 to 5 on pages 2 to 14.
The body runs to page 35; less the three landscape diagram pages that is 32 pages against 12 to 30. The core, sections 1 to 5, is 11 portrait pages against "about eight". Candidates for the appendix: the 6.8 table (its content is already in the cards and in the platform's 11.1 table), the 6.9 dependency table (GUIDE 6 allows it), and 10.3's tables; candidates for cutting: section 5's six-act list duplicates 7.1 almost line for line.
Fix: move the two tables and trim section 5's act list to one line each with pointers to 7.1.

### Minor

**F14.** R2. 4.2 second paragraph. "A 30-minute rehearsal at one action per 5 s holds about 360 interactions" is one session's count; A4's four sessions at 0.8/s give 1,440. Say which the samples come from (the actor on camera, probably) and keep 0.8/s for the fit check.

**F15.** R1. 6.7. P9 and P10 have ceilings but no cost line; P11 has one. Add which of the four costs each adds (P9: operations, kept true by hand; P10: complexity, a second reader of the API).

**F16.** R20. 7.2 step 5. `ik-3` appears here and nowhere else; the reading note says the packet names `ik-1` and `ik-2` for "its two record commands". Add `ik-3` to the reading note or name the stale edit's key in 4.1 R7 and 6.3.

**F17.** R11. Section 1. "adds three operator tools under a rehearsal flag (6.7)": only the reset is under the flag; P9 and P10 run outside the binary (6.7). Reword.

**F18.** R6. 10.4 after the table. "platform TH12, TH13, TH15, TH16 and TH21 reappear as TH9, TH11, TH12, TH13 and TH8": platform TH21 is a revoked member's queued command; demo TH8 is the reset racing a command, a different threat that merely reuses M40. Demo TH10 is platform TH4 and demo TH4 is platform TH1, neither mentioned. Correct the mapping.

**F19.** R6. 10.7 first line. "AR7 and AR10 moot for synthetic data": platform AR10 is losing the region; it is moot because there is no SLA (A8), not because the data is synthetic.

**F20.** R5, R18. Diagram 01 (page 10) uses B0 for the people's grouping and B2 for the canary's organization; section 7 and diagram 03 use B2 for the host's network, and B0 is in no boundary list. Give the context groupings ids that do not collide, and list them in section 7.

**F21.** R6. AR3 says the shell holds "three keys"; AS12 lists five items including the client secret and "role `reset`'s credential"; 10.5 says every database role connects with a 15-minute IAM token under one task role, so role `reset` has no separate credential. Reconcile AR3, AS12 and 10.5.

**F22.** R15. Section 9, the reset's guard: "412 unless every organization carries the synthetic label (A10)". The label is defined nowhere: a column, who sets it, whether the platform's schema has it. One sentence in section 8 or 6.7.

**F23.** R2. 11.2 provider line. "ten rehearsals × two takes × six": six attempts a take is unexplained; acts 3 and 6 at two attempts each are four. Say what the other two are (the patches? the spike?).

**F24.** R2. N2 and 11.3. "the maximum over ten restarts", "one a take, the maximum over ten": ten rehearsals of two takes are twenty takes. Pick ten or twenty.

**F25.** R16. 4.1 R4 says a later change MUST invalidate the approval; it appears in no act's happy path and in no "Captured" list, only under act 4's "Things go wrong". Either add it to act 4's captures or mark it a test rather than a scene.

**F26.** R3. Section 1 trades away "scale, availability and cost"; section 5 adds "generation latency". Align.

**F27.** R23. 6.1 ceiling. "Idle at four sessions, far below the platform's 300 KiB typed-props limit": the platform's limit is per view's props, not a function of sessions. State the real ceiling (the largest view's props, the six-row table) or say the ceiling is the platform's.

**F28.** R16. Section 2 cites the platform record "at its phase 2 commit `a6498dc`"; appendix B says "at the phase 7 commit this record cites". Name one pin; it decides which platform text the demo's D-, M- and TH-ids refer to.

**F29.** GUIDE appendix. Appendix lettering runs A, B, D; there is no C. Either letter D as C or add a one-line C pointing at 6.9's table.

**F30.** R13, R14. 3.1's first paragraph bolds six goals without their mechanisms (the bullets carry them); the positioning in section 5 is against "a staged product demo", a practice rather than a system the reviewer knows. Add a half-sentence of mechanism per goal, and consider positioning against the platform's own Retool line ("like the platform's demo on Retool would be ...").

**F31.** R8. 13.2 "Before build": "The load balancer's two subnets and the IPv4 charge: vendor claims to verify; 11.2 stands" is a claims-phase note, not a question. Move it to 13.1's debt line or cut it.

**F32.** R15. 6.7 P11 and 6.1 example. The reset "keeps sessions" and "bumps the membership epoch so open tabs refresh"; 6.1's example reads as if a session minted at epoch 7 is denied once the epoch is 9. One sentence on what an epoch bump does to a kept session (nothing; it makes the island re-query and drops results computed under the old epoch) would stop a builder invalidating Sarah's and Jordan's sessions at every reset.

**F33.** R18. Diagram 02's arrow labels are noun phrases ("pages, API calls", "publications, policy, audit"); the C4 rule asks for a verb and an object. Prefix the verb.

**F34.** R15. 7.1 act 3 step 2: "twenty synthetic rows" when the fixtures hold six events, two inventory rows and three expenses. Say whether these are the fixture rows or the platform's curated examples; N8's "one provider call that saw no evidence" depends on it.

## 4. Platform citations checked

Hold as attributed: platform D1 to D15 and their doors; C01 to C16 by name; R1 to R16 and N1 to N12 where cited; the fit check's 100 ms, 2 per organization, 8 per node, 20 waiting then 429 (platform 5.2, 6.3); 24 views and 8 operations a view (6.2); the 300 KiB props ceiling (6.1); eight slots and the 120 s lease (6.4); A17's 60 s; the F-id ranges cited for acts 1 to 5 (platform 7.1, 7.2) and F46 for the command (7.3); READ COMMITTED with locks (8) and the SERIALIZABLE publisher (6.2); 24-hour outcomes (A6); the 404 rule (9); TL0 to TL8, SI1 to SI6, AS1 to AS8, TA1 to TA10; M1, M3, M6, M7, M8, M9, M14, M16, M22, M24, M26, M30, M32, M40, M41, M42, M45; TH6, TH28, AR2, AR3, AR9, AR11; SLO1 to SLO5, AL2, AL6; the five migration steps; the Retool prior art.

Do not hold: A6 as "the platform's caps" (F2); "platform SR1 to SR12 hold" (F3); the record-schema check "from P3" (F9); the island's fan-out of two at a time attributed to platform 5.2 and 6.3 (F4); the catalogue reduction (F5); "platform TH21 reappears as TH8" (F18); AR10 moot "for synthetic data" (F19).

## 5. The three questions I would ask the author first

1. Which fixture state does a take of act 4 start from, and what do takes of acts 5 and 6 replay off camera before the recording runs? Section 8's `s4` already holds publication 1; 6.7 says states are captured at each act's start; 11.1 resets act 4 to `s4`.
2. Is publication policy keyed by organization or by space, and is Spend Review's publication under Finance's owner policy a scene the partner sees? Section 5's act 6 has no publish step; 7.2 introduces the policy in passing.
3. After the simulated lost response, does the island re-POST `ik-1` (7.2) or GET the outcome by key first (section 9), and how does P10 get a header-free capture when Chrome's HAR export carries the `Cookie` header on every authenticated request?

## 6. Verdict on readiness at L2

The record is a competent child of the platform record: its identifier discipline is unusually good, every number in the fixtures recomputes and agrees across sections 2, 4, 5, 7 and N6, the cost table's arithmetic checks line by line, the threat model is specific to what the demo adds (the reset, the operator's shell, the packet) rather than a copy of the parent's, and it says "unknown" where it is. It is not ready to pass the gate as it stands, for three reasons that a builder would hit on the first rehearsal rather than in review: the take procedure's fixture states cannot produce a take of act 4 (F1); two statements about the platform are false as written, the input caps (F2) and the security requirements declared to hold (F3); and the two mechanisms the YC partner's evidence rests on, the retry and the capture, are each described in two incompatible ways (F6, F7). Beneath those, the demo's own attack surface is undrawn (F10), the burst arithmetic needs its mechanism (F4), act 6's publication is missing from the scene list (F8), and the body is two pages over L2's range with diagram labels that do not survive print (F11, F13). None of this needs new design; it needs one consistency pass of about a day by the lead, then a re-read of sections 5, 7, 8, 9 and 11.1 together. I would advance it after that pass and not before.
