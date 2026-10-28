# Fresh review B: the Forma company app platform (L3)

Reviewer: a fresh reader with no prior knowledge of the project, reading as the senior engineer who would build it. Inputs: `design.md` (2026-10-22, Proposed), `reference/CHECKLIST.md` (R items), `reference/GUIDE.md` section 3, and the rendered pages (61; the diagram pages 12, 14, 18, 24, 26, 29, 31, 45 looked at for legibility, pages 16, 17, 52, 54, 55, 56 for layout and the page budget). Nothing else was read; "the definition", the contracts C01 to C16, VALIDATION and the review are taken as the record cites them.

Emphasis, as briefed: numbers and mechanisms traced across sections (the fit check in 5.2 against section 4 and section 11; section 7's flows against section 6's contracts and section 9; section 8's locks against 6.3 and the sequence diagram; 11.2's costs against the sizes in 6 and 5.2), then the reading experience.

Counts: 0 blocking, 10 major, 14 minor.

## Major findings

### B1. The control plane is both on and off the published-app path (R16, R4, R2; GUIDE 3 section 6 "correspondence")

Where: 5.2, 6.1 "Write paths", 6.7, appendix C, 11.4 row 2, versus 7.2 step 2 (F29), 7.3 step 2 (F44), 7.3 "Version adoption", diagrams 05 and 06.

Problem: 5.2 says "published apps serve from the workspace server's own reads of publication rows, so the control plane is off that path"; 6.1 says "Publications are the workspace's own `SELECT`"; 6.7 and appendix C classify the control plane as soft; 11.4 says with the control plane down "published apps serve (5.2), drafts wait"; the availability chain (99.65% warm) excludes it. But flow 2 has P2 "ask P3 for the current publication (F29)" on every open, flow 3 has P2 fetch "the pinned publication's record schema from P3 (F44)" on every record command, and adoption has "P3 checks compatibility (F44)". Diagram 05's F29 and diagram 06's F44 are drawn as synchronous HTTP calls with service identity. The sequence diagram, meanwhile, shows no P3 call at all for the command. Three descriptions of one path. If F29 and F44 are real calls, the control plane is a hard dependency of navigation and of every commit, the N2 composition is wrong, the 11.4 row is wrong, and the B3 blast radius is larger than 10.2 says.

Severity: major.

Fix: decide. The cleaner reading, consistent with D2 and 6.2 ("writes the adoption table"), is that the workspace server reads publication, record-schema and adoption rows itself under role `workspace` (read grant only); then delete F29 and F44 from the DFDs and the flow prose, or redraw them as reads of DS1. If the calls stay, move the control plane into the hard rows, recompute N2 (about 99.55% warm), and correct 5.2, 6.1, 6.7, 11.4 and appendix C.

### B2. The record transaction has two lock orders and two positions for the idempotency row (R4)

Where: 6.3 "Record store" versus 7.3 step 3, the sequence diagram (messages 4 to 6), section 8 "Writes", and section 9 "Idempotency".

Problem: 6.3 gives D5's order as: authorization; the idempotency outcome as an insert under the unique constraint (so a duplicate serialises on that row); `FOR SHARE` on membership and saved work; then `FOR UPDATE` on the record and the version check; then the writes. 7.3 and the sequence diagram do: outcome lookup (not an insert); record `FOR UPDATE`; then membership `FOR SHARE`; then insert record, outcome and audit at the end. Section 9 promises "a key in flight serializes on the uniqueness row and returns the first outcome", which only holds under 6.3's insert-first order. Under the sequence diagram's order, a concurrent duplicate finds no outcome, blocks on the record lock, re-reads version 1 under read committed and returns 409, not the stored outcome; G4's "a timeout retry with the same key yields one mutation" survives, but "returns the original outcome" (R8) does not for the in-flight case. Lock order also matters because revocation takes the membership row `FOR UPDATE`; the record must state one order so every code path agrees. "record `FOR UPDATE`: absent" in the sequence diagram is not a lock either; the racing-insert case rests on the record's unique key, which 7.3 mentions and 6.3 does not.

Severity: major.

Fix: one order, stated once in 6.3 and copied verbatim into 7.3 step 3 and the sequence diagram: authorize; insert the outcome row (payload hash included) under the unique index, a duplicate blocking here and returning the stored outcome after the first commits; membership and saved-work `FOR SHARE`, epoch and pin checks; record `FOR UPDATE` or insert under the record's unique key, version check; writes; commit. Section 8's paragraph then matches.

### B3. N1's "10x" is three times the design point, not ten (R2, R13)

Where: 4.2 N1, 4.2 load paragraph, glossary "design point", 5.2, 11.2 "At 10x", 11.2 cost table.

Problem: the design point is defined as the peak: "at peak all 100 seats, 10 interactions/s and 60 operations/s ... the design point" (4.2), and the glossary repeats "60 query operations/s ... at peak". N1 then defines ten times the design point as "100 organizations, 300 concurrent, 180 operations/s", which is ten times the 18/s average and three times the 60/s peak. 11.2 sizes everything at 10x from 180/s (18 in flight, three data tasks plus one, 360 primary reads/s), and the 10x cost column follows. At ten times the peak, 600/s, Little's law gives 60 in flight, eight data tasks at 8 slots, and the cost table roughly doubles.

Severity: major.

Fix: say which it is. Either N1 reads "ten times the average load with the same 3.3 peak factor, 600 operations/s at peak" and 11.2 is resized, or N1 says it is ten times the average and the design point's peak is restated as 60/s with 10x meaning 180/s average, 600/s peak. The sentence "the design point fits one PostgreSQL and one query node" is unaffected.

### B4. The canvas's 5-second refresh is a load the fit check never counts (R2, R13)

Where: 5.2 "Revocation within 5 s is the membership epoch checked on every request plus the canvas's refresh, at most 5 s apart"; 6.5 "the canvas re-queries at most every 5 s, the other half of N3's revocation bound"; 7.2 step 8; TH12 (M24 "the canvas re-querying within 5 s").

Problem: the revocation bound depends on every open canvas making a request at least every 5 s. The fit check assumes one interaction per seat per 10 s (60 operations/s at peak). If the refresh is six typed queries, 100 open canvases add 120 operations/s at peak, doubling the data-service load and the 120 primary reads/s in section 8; if it is one authorization ping, it adds 20 requests/s to the workspace server and 40 primary reads/s, still absent from 6.1's ceiling. The record never says which. A builder cannot size the data service or the edge's 100-requests-in-10-s client limit (a six-query refresh every 5 s is 12 requests in 10 s per canvas before any interaction) without knowing.

Severity: major.

Fix: define the refresh in 6.5 (recommended: one `GET /v1/session` carrying the epoch, no queries; results re-fan only on an epoch change), add its rate to the load paragraph in 4.2 and to the fit check, and check it against the edge limit in 6.6.

### B5. The six-view refresh cannot meet p99 2 s in three waves if each operation may take 1.8 s (R2)

Where: 5.2 "Latency", 6.3 "interrupted at the caller's deadline, 1.8 s for an interactive operation", 11.2 "one cold aggregate ... holds a slot up to 1.8 s", 13.2 "During build".

Problem: the refresh is six operations at two per organization, three waves. 5.2 derives 300 ms warm (3 × 100 ms assumed) and says the whole must sit "inside about 1.8 s after the round trip"; 6.3 gives each operation a 1.8 s deadline. Three waves of operations that may each run to 1.8 s is up to 5.4 s, so the p99 2 s target holds only when all six run concurrently or when a single cold operation is the whole budget. 13.2 then says "three waves at 900 ms", which agrees with neither 300 ms (5.2) nor 1.8 s (6.3). The record's critical latency SLO rests on arithmetic that disagrees with itself.

Severity: major.

Fix: state one budget: either the per-organization cap is 6 (one wave) for a refresh and 2 for everything else, or the refresh deadline is 1.8 s total with each wave at 600 ms, or the p99 target is restated. Make 5.2, 6.3, 11.2 and 13.2 carry the same numbers; derive the 900 ms or delete it.

### B6. The audit hash chain has three concurrent writers and no linking mechanism (R4, R21)

Where: 6.3 "The audit table is insert-only by role and event kind, `workspace` ... `control` ... `data`; each organization's rows chain by hash"; SI5; TH11 M22; 12.1.

Problem: a hash chain needs each insert to read the previous head. Three roles insert into one chain per organization from independent read-committed transactions (sessions and grants from `workspace`, governance from `control`, record events from `data`). Without a per-organization head row locked `FOR UPDATE` on every insert, concurrent inserts fork the chain; with it, every record command, sign-in and grant in an organization serialises on one row, which the fit check and the 50-commits/s hot-record ceiling do not include. The record says the chain is verified daily (12.1) but not how it is formed. SI5 names the enforcers as "row policies, insert-only grants, anchors", none of which links rows.

Severity: major.

Fix: choose and state the mechanism in 6.3: either one chain per (organization, writer role), each role linking its own rows under its own head row, the anchor covering the three heads; or a sequence number per organization with the chain computed by the anchor job over the ordered rows. Name the cost (a lock, or a window in which the chain is not yet formed) and update SI5 and M22.

### B7. "Lockstep" shared contracts versus a callee-first rolling release (R22, R4)

Where: 6.7 "in lockstep because one binary ships them (D11)", appendix C last row, versus 11.1 "deployed callee first ... Three waves ... each baked 30 minutes".

Problem: one binary does not make the roles move together. 11.1 rolls the data service, then the control plane, then the workspace server, then the workers, each through three baked waves, so an old workspace server talks to a new data service for an hour or more per release, and a new one to an old control plane. The `AppDefinition` schema, the receipt and the service-token format cross those boundaries during the window; 6.7 covers only "an old tab meets a new API" and a schema-diff test for the browser, and 11.1's "encoding compatibility makes the rollback safe" names no rule for the internal contracts. A builder does not know whether the receipt and the internal request types need the section 9 compatibility rules or may change freely.

Severity: major.

Fix: in 6.7 and appendix C, replace "lockstep" with the truth: the internal contracts follow section 9's rules (requests backward, responses forward, fields only with defaults) for one release's window, checked by the same schema-diff test in CI; say what an old workspace server does with a receipt field it does not know. Or deploy all roles in one wave and say the mixed-version window is seconds.

### B8. The data-flow diagrams are not legible at print size (R5, R18; GUIDE 4 "the rendered pages remain the final judge")

Where: page 24 (diagram 04), page 26 (05), page 29 (06); to a lesser degree page 14 (02), page 45 (08), page 18 (03).

Problem: on the landscape pages the DFDs' arrow labels and node bodies render at roughly 4 to 5 points, measured against the 8-point running footer; the titles are about 6 points. The labels are the content a threat reviewer needs (payload, protocol, authentication, data class, and the F-numbers that 10.4 cites); at this size they cannot be read on paper, and the eight-boundary legend text is smaller still. Diagram 06 occupies under half of its page's height because its layout is very wide, so it is scaled down although there is room. The container and deployment diagrams land near 6 points, readable with effort; the sequence diagram is the only one comfortably readable. Diagram 03 sits on page 18 after a page 17 that carries two paragraphs, which GUIDE 4 accepts, but the reader meets three nearly blank portrait pages in the record for the same reason.

Severity: major.

Fix: for 04, 05 and 06, reduce the natural width (two rows of processes instead of one; the stores in a second band; labels of at most three lines with the F-number first) so the compiler's scale on the landscape page keeps body text at or above 7 points; or split 05 into sign-in and run. For 06, stack the browser band above the VPC band so the diagram is nearer 1.5:1. Check the pages again after the compile.

### B9. The body is about 47 pages against the L3 range of 24 to 40, and the overrun is repetition (R17, R15)

Where: pages 1 to 55 minus eight landscape pages; sections 5 to 11.

Problem: section 13 ends on page 55 and the appendix fills 56 to 61, so the body is about 47 portrait pages. The weight is where GUIDE wants it (5 to 7 and 10), but the same mechanisms are told in full many times: revocation within 5 s in 5.2, 6.1, 6.5, 7.2 (twice), 8, SI4, TH12, 11.4 and the glossary; the idempotency transaction in 6.3, 6.5, 7.3, 8, 9 (twice), TH16; the fAuth token checks in 5 step 1, 6.1, 7.2 step 1, 9, 10.5, TH3; the Northstar numbers (1,140,000; 900,000; 114,000) in 2, 3.1, 4.2, 5, 6.3, 7.2 and 11.4; isolation's two walls in D3, 6.6, 8, SI1, TH1 and 12.2. Each retelling is slightly different in wording, which is how B1, B2 and B5 arose. The appendix is at its 6-page limit, so there is nowhere to move material; it must be cut.

Severity: major.

Fix: tell each mechanism once where it is decided (6.x), and elsewhere cite it in a clause. Candidates: 7.2 step 1 and 9's sign-in paragraph collapse to one sentence each pointing at 6.1; 8's "Writes" paragraph and 9's idempotency paragraph each lose their restatement of D5; 6.5 and 7.2's late-response bullets merge; the worked numbers appear in 2 and 7 only. That alone recovers five to seven pages.

### B10. The `AppDefinition`, the one-way door the whole record rests on, is never shown (R14, R15; GUIDE 3 section 5 and 6 "a worked example with concrete values")

Where: D1, 6.2 "Admission", 6.3 "Query engine", C04 as cited.

Problem: the record cites C04 for what a definition is and gives admission limits for it ("at most 24 views, 8 operations a view, 256 KiB of definition, 500 rows a page"; "the grammar has no URL, path, id or grant type"), but a reader given this document never sees a view, an operation, a record schema or a declared command, nor what "typed props" are resolved from. The claim that the model "proposes data, never code" and the claim that "a semantic diff is exact" cannot be judged, the admission card cannot be built, and the alternatives in 12.2 D1 compare a visible markup path against an invisible one. R9 forbids a pasted schema; it does not forbid an eight-line fragment.

Severity: major.

Fix: in 6.2 or 6.3, one fragment of Maya's definition with concrete values: one view (`summary` over `events`), its one operation (`sum(bytes) where host = ? and ts in [a, b)`), the observation record schema (three fields, bounded types), the `needs_context` command with its transition; and one sentence on how the grammar's closed types make "no URL, path, id or grant" true by construction.

## Minor findings

### B11. The cold-aggregate fallback is "at publication" in 5.2 and "at admission" in 12.2 and RK5 (R2)
Fix: one word, one place; admission is the earlier gate and the one 6.2 owns.

### B12. SR9, AR1 and M14 disagree on the second person (R21, R6)
SR9: "destructive changes two-person after 24 hours"; M14: deletion and key disablement "take two people"; AR1: "platform-wide destructive actions wait 24 hours with notice instead of a second person". A builder of SR9 does not know whether to require an approver. Fix: SR9 reads "two-person, or with one operator, a 24-hour hold with notice (AR1)"; M14 cites AR1.

### B13. Section 9's error table and the `command` contract do not match the flows (R15, GUIDE 3 section 9)
The Queries row lists 404, 422, 429 but 9's prose and 11.4 return 503 on a cold query whose store or key is unreachable; the Record commands row lists 412 and no case in the record produces it; 6.3's `command(declared command, target, expected version, values, key)` omits the principal, epoch, work id and app revision that D5's transaction checks and that diagram 06's F45 and the sequence diagram carry; the canonical payload hash that makes "same key, different payload" a 409 is stored nowhere the record names. Fix: add 503 to Queries, drop or justify 412 on commands, complete the signature, and say the outcome row holds the payload hash.

### B14. 11.2's arithmetic does not follow its own sizes in three places (R2)
(a) At 10x, 11.2's sizes are four data tasks (16 vCPU, 64 GiB), four workspace (4, 8), six workers (6, 48), two control plane (1, 2): 27 vCPU and 122 GiB, which at the quoted unit prices is about $1,200 a month, not $1,030. (b) KMS: "12 keys" against ten organization keys, platform keys for RDS, backups and anchors, and one asymmetric key per role, at least 15. (c) Six worker tasks of 8 GiB hold 48 GiB; 30 parser sandboxes at 2 GiB need 60. Fix: recompute or restate the sizes; the total moves by about $200 at 10x.

### B15. Numbers without a derivation (R2)
260 generation attempts and 1,320 explanations a month; 10 million CloudFront requests; 100 GB egress; 10 GB of logs; 300 metric series; "our two roles at 99.9% each"; the N3 table's "each operation at p99.8" (1 − 0.998^6 ≈ 1.2%, so about p99 for the six) is a derivation worth one clause; the ALB (99.99%) is missing from the availability chain while CloudFront is in it. Fix: one line each from A1, A2, A5 and A18 (for example 10 organizations × 2.5 builders × about 1 generation a business day ≈ 550, or whatever the figure is), and add the ALB.

### B16. Wrong and missing cross-references in 11.1 (R22)
"Each service keeps 100% capacity, a new task joining before an old one drains (6.4)" points at the Workers card; the static-stability statement is 11.1's own. "Three waves behind a weighted target group" for the data service and control plane implies an internal load balancer per role that neither diagram 02 nor 08 shows (08 shows security groups and direct calls). Fix: correct the reference; say how internal roles are canaried (an internal ALB, or ECS task-set weights).

### B17. Appendix D threats are clauses, not statements, and have no existing-control column (R6)
TH8 to TH28 give "the threat in one clause" without source, prerequisites or impact, and fold existing controls into the mitigation cell. TH4's "existing control" is the prototype's browser cap, which is not a control of this system. Fix: at least for the four High-impact rows TH8 to TH11, write the full grammar and separate the existing control; mark TH4's existing control "none".

### B18. The summary's counts and doors (R11)
"one binary in four roles over two stores" while section 7 and 6.6 name three stores (DS3 key management); D15 is "one-way per organization once its data lives there" and the summary names only D1 to D4, where GUIDE asks that a door that closes later be named with its closing event. Fix: "over two data stores and one key store"; add "D15 closes per organization at its first real row".

### B19. Table columns wrap letter by letter (reading experience; GUIDE 4 "text fits its box")
Page 16 (5.3): "ID" renders as "D / 3", "Serves" as "Serv / es", "Section" as "Se / cti / on"; page 52 (12.2): "Loses against" as "Lose / s / agai / nst"; page 54 (13.1): "Rank" as "Ran / k", "RK1" as "R / K1". Fix: widen the ID and short columns or drop the "Binds demo" column into a footnote; the tables are the record's most-read pages.

### B20. Two alternatives lose against rulings and assumptions, not goals (R7)
12.2's table: G "loses against K4, A11"; K "loses against A8, N10". Fix: name the goal or non-goal (G for G5 or N7 via the adapter; K for N10 alone, with A8 as the constraint that eliminated it).

### B21. The edge's ceiling is checked against the wrong peak (R2)
6.6: "unchecked against a peak near 10 requests/s", while 4.2's peak is 60 operations/s, each a request, before page loads and the 5 s refresh (B4). Fix: "near 100 requests/s" once B4 is settled.

### B22. The 20-waiting cap is defined for queries and applied to commands (R2)
6.3 defines "429 past 20 an organization" for query operations; 11.2 applies "capped at 20 waiting with 429" to commands on a hot record, which 6.3 says wait on the row lock. Fix: say whether commands queue, and where the cap lives.

### B23. Retention appears in two lists that do not match (GUIDE 3 section 8)
10.6's dictionary keeps "jobs 90 days" and "anchors a year"; section 8's retention paragraph has neither. Fix: section 8 is the owner; 10.6 cites it.

### B24. Diagram numbering jumps from 06 to 08 (GUIDE 4)
The sequence diagram is never called diagram 07; a reader looks for the missing one. Fix: "Diagram 07, the sequence diagram" in 7.3's narration.

## Items judged to pass

R1 (reason and cost together: the D-table's Cost column, every card's Cost line, 12.2's Good/Bad); R3 (N1 to N12 each with a number or a reason, ranked N7, N9, N6, N5, N3 matching the optimisation sentence); R7 in shape (decision records for D1 to D4 with confirmation lines; do nothing and reuse present); R8 (bucketed, with candidates and a current choice); R9 (no dumps; the adjectives carry numbers); R10 ("assumed until measured" is used honestly throughout); R12 (6.7's pattern paragraph; extension points name their change); R13 (scenario, fit check, D-table, Retool positioning, 10x); R14 (bold goals, numbered phases with terminal lines, worked values); R16 (traceability by ID is thorough); R19 (section 2's six-step mechanism with components and numbers); R20 (13.3); R21 (SI1 to SI6 with enforcers, the entry-point table, the dictionary, JIT and breakglass with an outside reader, fail-closed per control, SR1 to SR12); R22 in structure (bridge states, the single rollback metric, the upgrade-downgrade test); R23 (the spike reported and the next one named; every card has a ceiling with its "because"); R24 (four six-part scenarios compressed into one paragraph, change and fault included; risks on 10.4's scale).

## Three questions for the author

1. Does the workspace server call the control plane when a member opens an app and on every record command (F29, F44 as drawn), or does it read the publication, record-schema and adoption rows itself (5.2, 6.1)? Everything about N2, the soft/hard table and the B3 blast radius turns on the answer.
2. In the record transaction, is the idempotency outcome row inserted before the version check and the record lock (6.3, section 9's in-flight promise) or looked up first and inserted last (7.3, the sequence diagram)? And is the lock order membership-then-record or record-then-membership?
3. Is N1's 10x ten times the 60 operations/s peak design point or ten times the 18/s average, and what does a canvas send every 5 s: six queries or one epoch check? The fit check, the data-service count and the cost table depend on both.

## Verdict on readiness at L3

Not ready to deliver as it stands, but close, and nothing found needs a redesign. The architecture is coherent and well chosen: one binary in four roles with disjoint database grants, a typed definition the model can only propose, a publication record only the control plane writes, receipts on every number, and the organization id as the first key everywhere; section 10 is the strongest part, with six invariants whose enforcers are named, an honest accepted-risk list owned by the head, and fail-closed stated per control. What blocks a build today is that the critical paths are told several times with differences a builder cannot resolve: whether the control plane is on the read path (B1), the order of locks and the position of the idempotency row (B2), what 10x means (B3), the refresh load the fit check never counts (B4), and three different budgets for the six-view refresh (B5), plus two mechanisms with no stated enforcer (the audit chain, B6; internal contract compatibility across a rolling release, B7). The remaining majors are presentation: the DFDs cannot be read on paper (B8), the body is about 47 pages against 40 because the same mechanisms are retold up to nine times (B9), and the one-way door the record is built on is never shown (B10). One revision pass that settles the five contradictions in their owning sections, cuts the retellings to citations, relays the three DFDs and adds an eight-line definition fragment would, in my judgement, pass the gate; I would then want to re-check B1 to B5 against the corrected pages before signing.
