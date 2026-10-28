# Junior-reader check: the annotated edition of forma-generative-app-platform-v2

Reader: a developer with two years' experience, joining to build the platform, reading a system design document for the first time. Read: `design.md` with the notes of `annotations.md` placed at their keys, the glossary, the exercises and the task map, and `reference/TEACHING.md` (the section notes and the eight-lesson course). Not read: briefs, STATUS, reviews, intake, input.

The short version: the record is disciplined about defining its own identifiers before it uses them (I checked: no SI, TH, TA or AG identifier appears in the record before its table, and the only AR before 10.7 is inside section 10). The teaching layer is not: the notes and the exercises lean on identifiers and sections that come later, which is where most of my trouble came from. The notes that explain a mechanism (6.2, 6.3, 5.2, 4.2, TH10, 11.3) are very good; about a third of the notes restate the sentence they sit next to.

## 1. What I still could not follow after the notes

One line per place, by section.

- **Cover, "How to read this record."** The note tells me which document K, C, P, H, I, FORMA and BUILD come from, but I cannot open any of them, so "(P06; C04, C06)" at the end of a requirement stays a bracket of letters; I never learn what C04 or C11's "ten permissions" actually say.
- **Cover.** "L2" and "L3 strength" are never explained (document levels, I guess); nothing in the layer says what a level is.
- **1.** "fAuth behind an identity adapter": fAuth is not named as a product until section 2's constraints, and "adapter" as a pattern is only explained in the 6.7 note.
- **2.** Forma the framework and Forma the platform share a name; the first paragraph switches between them and I had to read it three times. "mountMarkup" versus "reconcile", "0 FAIL gate", "loopback" and "fixture" are not explained anywhere.
- **2.** "The compiler admits a finite subset of TypeScript for server rendering": the note gives the BUILD-610 example, but I still do not know what I may and may not write when I build a renderer.
- **3.3, A13.** "Where the two product definitions differ": which two? The record names one controlling definition; the other is never identified.
- **4.1.** "Programmatic parties are callees" stopped me cold; no note.
- **4.2.** "Scenarios (source, stimulus, environment, artifact, response, measure)" names a six-part form and then the four scenarios do not visibly follow it; no note.
- **4.2, SR list.** SR13 comes before SR12, and every SR cites M-numbers that only mean something after 10.4 and appendix D; the note says "read lightly now", which is honest but is not help.
- **5.2.** The availability chain multiplies "our two roles at 99.9% each"; the note explains the multiplication but nobody says where 99.9% for our own services comes from.
- **5.2.** "Adopts pages the workspace server renders" (and 6.5 "adopts what the server rendered"): this "adopt" is never defined and collides with "adoption", which the record defines as moving saved work to a new publication; I read it as the wrong thing for a while.
- **6.1.** "Signed with the role's hourly KMS-certified key": the 10.5 note repeats "certifies hourly" but neither says what certifying an in-memory key with a KMS asymmetric key means mechanically.
- **6.2.** "Writes the adoption table, the earlier publications this one is record-schema compatible with": I could not picture the table (rows of what, keyed how) and no note helps.
- **6.3.** "The deadline interrupt a CI test" is not a sentence I can parse; "a per-query S3 session scoped to the organization's prefix" names a mechanism (how do you scope an S3 session?) that nothing explains.
- **6.5.** "The fifth primitive that imports `createResource` or the HTML runtime": Forma internals assumed known.
- **7.** The F-numbers only mean something with the picture; reading the text with the notes, "F12 is the only arrow leaving the VPC" is a claim I have to take on trust because I cannot see which arrow F12 is without the diagram open beside me.
- **7.3, step 2.** "Rejects fields and transitions the pinned publication's record schema, read from DS1 (F44), does not declare": garbled, could not follow.
- **8.** "Primary only: read-after-write, monotonic": read-after-write is in the glossary, monotonic is not. "Every index is B-tree, local": local as opposed to what? "Past that, a cap, not a split": split of what?
- **9.** "Fields added only with defaults, tags never reused": which tags? (Wire-format field numbers, I guess.) "16 bytes of entropy" is fine; the tags are not.
- **10.2.** "SigV4 with the organization's context" and "the SDK" are not explained; the note on 10.2 explains trust levels well but skips the entry-point table's authentication column.
- **10.4, TH1.** "The platform ignores the slug header, which fAuth itself honours as a fallback": what slug header? First and only mention.
- **10.4, TH3.** "The refresh-cookie bridge unused": what bridge? First and only mention.
- **10.4, preface.** "A deferred one lowering nothing" took me three reads to parse as "a deferred mitigation does not lower the rating".
- **10.6.** Does the platform have a client secret or not? 6.1 says one `oauth_clients` row; the 10.6 secrets table says "a public PKCE client with no secret" and, two columns later, "a new client row and secret"; the glossary says "a client fAuth knows by a row and a secret". I could not resolve it.
- **11.1.** "A tenth of requests plus all of the canary name's": "canary name" is not a term anywhere else (canary organization is).
- **11.2.** "Not in the latency SLIs": SLI is never defined; the glossary has SLO only.
- **11.2.** ALB "capacity units" are not explained; the cost line cannot be rebuilt from the record alone.
- **11.4.** WAF "count mode" is not explained.
- **12.2.** "In Tan's framing" names a person I meet only in appendix B; "json-render-style libraries" names a category I do not know.
- **Appendix D.** Fine once 10.4's note was read; the only trouble was that the notes in sections 4 to 8 had been sending me here all along (see 2).

## 2. Words used before they were defined

In the record itself (the definition arrives later in the record or only in a glossary):

- **fAuth** (1; explained in 2 as a sibling product). **Pooled**, **control plane**, **Parquet**, **DuckDB** (1; D3, 6.2, appendix A and the glossary tab). The hover glossary covers Parquet and DuckDB; it does not cover "control plane" until appendix A.
- **Row-level security** (G1 wording in 3.1; defined in 8 and appendix A). **Compare-and-set** (G2; defined in the D14 note and glossary, 6.1). **Idempotency key** (G4; defined in 9 and appendix A). **Half-open** (G3; glossary keyed to 7.2).
- **Spike** (2, "the spike's canvas"; defined in appendix A). **Fixture** (2; never defined). **One-way door** (1; defined in the 5.3 note and TEACHING's note on 1; the record defines it in 5.3).
- **Semantic diff**, **patch on a base revision** (R5 in 4.1; D9 and glossary). **Membership epoch** (N5; appendix A). **SERIALIZABLE** (SR6; explained at 6.2). **Hash-chained**, **anchored** (SR10; 10.6). **SBOM**, **SHA-pinned** (SR8; glossary keyed to TH6). **CSP** (SR13; glossary keyed to TH25). **M1 to M51** (SR1 to SR13 in 4.2; 10.4 and appendix D).
- **Typed props** (5 step 3; appendix A). **Nonce CSP** (5, "Prototyped"; glossary at TH25). **RDS Multi-AZ, CloudFront, S3, KMS** (5.2 availability paragraph; D15 in 5.3 and the glossary at 6.6). **Static stability** (named in the 5.2 note; defined in 11.1 and appendix A).
- **Adopt** in the hydration sense (5.2, 6.5; never defined; collides with "adoption"). **FOR UPDATE** (6.1 example; explained in 6.3). **Object Lock** (6.3; glossary at TH11). **Goodput** (11.2; appendix A only). **SLI** (11.2; never). **SigV4** (10.2; never). **mTLS** (10.5; the note at 10.5 expands it, so this one is fine).

In the notes and exercises (the layer uses an identifier before the record has introduced it; the hover link helps only if I know to hover):

- **SI1, SI4** in the notes on D6, SR7, 6.1 and 9, and in the exercises after 6.1 and 8; SI is defined in 10.2.
- **TH4, TH5, TH9, TH10, TH15, TH16, TH29** in the notes on 2, SR7, D10, 6.2, 6.3, 7.1 and the exercises after 6.4, 7.1 and 7.2; TH is defined in 10.4 and appendix D.
- **TA5, TA6, AG4, AS2** in the note on 6.2 and the exercise after 6.2; defined in 10.2 and 10.3.
- **AR5, AR10** in the notes on 5.3 and 6.4 and the exercise after 1; AR is defined in 10.7.
- **M44, M30, M2** in the notes on 6.4, 7.1 and TH1 and the exercise after 7.1; M is defined in 10.4 and appendix D.
- **SA2** in the SR3 note; defined in 10.1.
- **VPC** in the SR3 note; glossary keyed to 7.
- **Little's law** is used in 5.2 and defined in the same note; fine. **Write skew** is defined in the sentence that uses it; fine.

## 3. The exercises

Answerable by lookup alone (the record or the note beside it holds the answer in nearly the same words):

- **After 1**: the D15 row and the 5.3 note say "two-way; one-way per organization" and why; the record's own summary names the exception. Except that the exercise says "without reading 5.3 yet", and at that point the *why* (AR10, backups stay in the region) is not available anywhere before 10.7, so as placed it is not answerable and as a lookup it is trivial.
- **After 2**: step 3 of section 2 gives both rates and names the denominator and unit.
- **After 3.3**: A3 lists the chain; one multiplication; the 3.3 note already does the exercise.
- **After 4.2**: the 4.2 note is the answer, including p99.83.
- **After 7.1**: TH15 and M30 say it verbatim; the only work is finding appendix D.
- **After 7.2**: the "Revocation lands during a queued query" failure case in 7.2 is the answer.
- **After 7.3**: section 9's idempotency paragraph is the answer, including the IETF comparison.
- **After 9**: section 9's error paragraph is the answer.
- **After 10.5**: 10.5's service-identity paragraph and AR3 are the answer.
- **After 10.7**: AR9 and the 10.7 note are the answer; the one inference, that an anchor on the commit path lowers the composed availability, is small.
- **After 12.2**: the D1 record in 12.2 lists the three things it loses against.

Answerable with real thinking, and the note made it possible (these are the good ones): after 5.2 (recompute at 200 ms), after 5.3 (D4 one-way, DuckDB not), after 6.2 (make the case for L), after 6.3 (swap the lock order; the best exercise in the set), after 6.4 (sandbox versus in-process), after 8 (what a naive cache key breaks), after 10.2 (why the island is outside the trusted base), after 10.4 (TH5 without a digest), after 11.1 (does the wave roll back), after 11.3 (page versus ticket), after 13.1 (argue RK5 to High).

Not answerable at the point they are placed, or not fully answerable even with the note:

- **After 6.1** asks me to name "the invariant"; SI4 is defined in 10.2, four sections later. I could name N3's row, not the invariant.
- **After 8** asks for "the invariants" by name; same problem (SI1, SI4 come in 10.2).
- **After 7.1** and **after 7.2** ask for "the row and the control" and "SI4, TH21"; TH and SI are both ahead of me. The behaviour I could predict; the identifiers I could not.
- **After 11.1** needs SLO3 and AL3, which are in 11.3, the next section.
- **After 11.2**: I rebuilt the vCPU and memory terms ($384 and $175) and the data service's $340, but the "$565 with the disk" needs a per-task disk size the record gives only for the data tasks, and I do not know how the platform bills ephemeral disk, so I could get "about $560", not the record's number, and I could not have told whether I was wrong.
- **After C**: I could multiply the vendor figures, but the two 0.999 factors for our own roles are asserted, not derived, in 5.2, so half of the exercise is copying.
- **After 1** as written ("without reading 5.3 yet"): see above; the reason is not available before 10.7.

## 4. Notes that repeat the record instead of explaining it

- **1, "Why the summary reads like a contract"**: lists the five sentences of the summary again; the "at the expense of" half is useful, the rest is a table of contents.
- **3.1, first note**: restates the seven done-when clauses; the second note (the failure sentence) is the one that teaches.
- **4.1, "Notice R14 and R15"**: paraphrases R14 and R15 with no added mechanism.
- **5.1**: repeats the record's own "What matters" paragraph under the diagram almost word for word.
- **6, "How to read a card"**: repeats the card headings in order; TEACHING's note on 6 already says the same thing, so I read it three times (record, method note, design note).
- **6.7, "Hard versus soft"**: repeats 6.7's first paragraph; the note on "a timeout is a refusal" below it is the one that explains.
- **7, "The diagram alphabet" and "Read an arrow label as three checks"**: both repeat section 7's opening paragraph and TEACHING's identifier paragraph; only the B7 sentence adds anything.
- **9, "Read the error column as policy"**: a near copy of the record's error paragraph with the RFC 9457 link added.
- **10, "How to read the section"**: the record's own first sentence, 10.4's preface and TEACHING's note on 10 all say this already.
- **10.3, "Attack goals group the rows"**: restates the AG table's first row.
- **10.6, "Three data classes and no Public"** and **"Every control fails closed"**: restate the table and the list; the exception about live sessions is also already in the record's sentence.
- **11, "One operator who sees no data"**: restates 11's first sentence.
- **11.1, "The reductions table"**: restates G1.
- **11.4, "Notice the last row"**: quotes the row and adds one clause.
- **12.1**: repeats the drawbacks list verbatim and then says "if you cannot write this paragraph..."; the second half is the teaching, the first half is a copy.
- **12.2, "Read a record as five parts"**: repeats TEACHING's note on 12.
- **13.2**: restates the three buckets and the examples already in the section.
- **13.3**: repeats TEACHING's Lesson 8 exercise nearly verbatim.
- **D**: repeats appendix D's own preface.
- **TH1**: restates M1 and M2 under a heading.

A general pattern: wherever TEACHING's grey note and the blue note sit beside the same heading (6, 7, 10, 12.2, 13.3), the blue one tends to say the grey one's content again in this design's words instead of pointing at the one thing in this design that is unusual.

## 5. The three things the layer taught me best

1. **When to use an isolation level and when to use explicit locks, and why the lock order matters.** The 6.2 note (write skew across three premise rows, so SERIALIZABLE; the record store locks every premise row, so READ COMMITTED is enough) and the 6.3 note (authorization, then the outcome under the unique constraint, then membership `FOR SHARE`, then the record `FOR UPDATE`, and why outcome must come before version), read with the sequence diagram and the exercise after 6.3, are the clearest explanation of transactional design I have read. I could now explain to someone else why a retry with a stale version must get its original outcome and not a conflict.

2. **How to land requirements on boxes with arithmetic.** The 4.2 note (3/s, 18 operations/s, 60/s at peak, p99.83 for six operations), the 5.2 note (Little's law, 6 in flight against 8 slots, the second task for availability not load, 99.65% composed, and 3.65 hours a month that one 4-hour restore would overspend) and the exercises after 4.2, 5.2 and C turned numbers I would have skimmed into numbers I can rebuild and argue with.

3. **A cache key is an authorization decision, and every surface a result can leave by needs a re-check.** The 6.3 note on receipts as the cache key, the TH10 note, the exercise after 8 and the task-map row for P11 (list every surface: search, cache, export, link, queued job) gave me a way of thinking about leaks I did not have: not "is the data encrypted" but "which answer could be served to the wrong scope, or after a revocation, and where is the re-check".

Runner-up: the 2 note that maps each numbered failure in the problem statement to the decision that answers it. It is the first time I have seen why a context section is written as numbered mechanisms, and I would copy that form.
