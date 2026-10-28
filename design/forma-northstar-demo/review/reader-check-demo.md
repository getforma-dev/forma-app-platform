# Reader check: the Northstar demo's edition, read cold

| | |
|---|---|
| **Readers** | 1. A YC partner with an engineering background: twenty minutes with the edition beside the six-act recording, deciding whether the demo proves what it claims. 2. The engineer who builds the demo, reading to start work on Monday. |
| **Read** | `design-demo.md`, top to bottom in each pass, and the four diagrams its image lines name (`01-context.svg`, `02-containers.svg`, `03-flow-demo.svg`, `04-flow-operator.svg`), rendered at full size. In the engineer pass only, `design.md`, to test chapter 9's map row by row. Not opened: `STATUS.md`, `intake.md`, `review/`, `input/`, the notes, `reference/`, the other briefs, and the platform record, so the map's third column is untested. |
| **Length** | 6,100 words by `wc -w`, about 5,950 of running text: front matter 209, summary 375, two pictures 477, acts 1 to 6 410, 350, 549, 398, 310 and 706, chapter 7 1,316, chapter 8 584, the map 263. |
| **Reading time** | Partner pass, about 34 minutes: 23 for the prose at 250 words a minute, 2 for the map's table, 9 for the four diagrams (02 and 03 are dense and need zooming). The twenty minutes run out in act 6, at diagram 03, before chapter 7. Engineer pass: the edition again plus the record's 20,700 words for the map, about two and a half hours. |
| **Date** | 2026-10-23 |

Marks: **[P]** the partner stumbled, **[E]** the engineer, **[P+E]** both. "Record N" names where the engineer pass found the answer in `design.md`; where no record section is named, the record does not answer it either.

## 1. What I could not follow

In reading order, one line per place.

**Front matter and summary**

1. [P] Summary, "the pins of what ran": "pin" is never defined, and act 1's "at its pin" (a commit) and act 4's "pinned to the publication" are two different senses.
2. [P] Summary, "at the expense of scale, availability, generation latency and cost": one process on one host at $170 a month is the cheap shape; I could not see what cost is given up.
3. [P] Summary, "excluded from acceptance": acceptance by whom, against which test, is never said; the phrase recurs in acts 3 and 6 and chapter 7.

**The system in two pictures**

4. [P] "The platform's four roles run as one program": the roles are named only after diagram 02, and "two outside services see opposite halves" waits two paragraphs for its halves.
5. [P] Diagram 01: the canary group's title runs out of its box; "another member on the Internet (group" is crossed by the box edge and cut at the image edge.
6. [P+E] "Look for two paths through one gate": in diagram 02 the background path never reaches the workspace server, since the workers claim jobs from PostgreSQL. I could not find the gate on the left path.
7. [P] "one task role can connect as all of them": "task role" is undefined; only chapter 7's "a Fargate task" hints that it is an ECS IAM role (record 10.5).
8. [P+E] Diagram 02's "(D1, D10)", "(B5)", "A6's limits" and "Host's edge (P1)", and diagram 03's "not a hop (D1)": the edition's only D1 is the platform's typed definition, and the demo's own D, A and B numbers are never introduced (record 5.3, 3.3; B5 is drawn nowhere).
9. [P] Diagram 02, "the canvas island" and "the composer chat": "island" is never defined, "composer" only in act 3.
10. [P] "about 0.8 actions a second": no basis given (record 5.2: four sessions at one action every 5 s).
11. [P+E] "two running and four waiting": why two is not said (record 5.2: the data service admits two of an organization's operations at once).
12. [P] "Four actors at once would queue 22 queries and see two errors": no basis for 22 or for two (record 5.2: 24 operations, two running, a queue of 20 past which requests get 429).

**Act 1**

13. [P] Step 4, "A command that matches nothing creates nothing": no command bar has been introduced, and from act 6 on "command" means a record command.
14. [E] Trade-off, "The platform's minimal login is the fallback": never described; I could not tell how it supplies `tenant_id` (record 5.3 D4 and 12.2 option E name it; nothing describes it).
15. [P] Trade-off, "a login that is not a reduction": the test for a reduction, a shortcut with a named seam, is given only in chapter 7.

**Act 2**

16. [P+E] Step 3, "an exact inventory entry before a network range": I read a match precedence but could not connect it to act 4's "matched" (record 7.1 repeats it; nothing defines the rule).
17. [P] The refusal, "The evidence keeps one refusal": in a chapter called "bringing evidence", where evidence is Maya's files, this evidence is the packet.
18. [E] "The sandbox dies at its memory cap": is the 50 MiB expanded cap a byte count or that memory cap, whose size the edition never gives? (Record 11.2: a 2 GiB cap; record 9 lists the oversize file as a 422.)
19. [P] Check first, "with the caps far above the fixtures, it is a real refusal, not a staged one": I could not follow why distance from the fixtures makes the refusal real.

**Act 3**

20. [P+E] Step 1, "summary, table and timeline are retrieved": this is Northstar's first app and nothing is published; retrieved from what? (Record 7.1: "Northstar's index"; nothing says what it holds at `s2` or who put it there.)
21. [E] Step 3, "a separate preview store": not one of diagram 02's two stores.
22. [P+E] End, "five revisions and one call to the model": two patches arrive, and the composer is what makes patches, yet the model is called once; I could not tell how the patches are written (record 7.1 repeats the count; nothing gives the mechanism).
23. [P] "Nor is the denominator hidden": "Nor" follows no negative; I read it twice before connecting it to the handler argument (record section 2, item 1).
24. [P+E] "it cannot pass the generation gate" (again in chapter 7, reduction 5): the gate is never defined.
25. [P] Check first, "names the registry version" and "the platform review's prototype canvas": the registry is explained only in chapter 7, "the platform review" never.

**Act 4**

26. [P+E] "240,000 matched, 900,000 unmatched, all to `198.51.100.7`": matched against what? With all five events going to one address, I could not see how their bytes split.
27. [E] Step 2, "input contracts": undefined.
28. [E] Step 3, "the timeline's late answer from epoch 11 is dropped": what makes the timeline late, every take, on camera? (Record 6.5 repeats it; nothing says.)

**Act 5**

29. [P] "starts his own work on another host": after "one process on one host" I pictured a second server; it is the monitored machine `ops-runner-04`.
30. [P] Step 2, "a labeled snapshot": this "labeled" is not the simulated-or-seeded label of the "What is labeled" paragraphs.
31. [P+E] Step 3, "210,000 bytes, 30,000 unmatched": five of the six CSV rows are `build-worker-17`'s, so `ops-runner-04` has at most one event; I could not see how one event splits.
32. [P] Step 4, "a result of his still queued included": I could not parse it (record 6.1: "a queued result of his at delivery").

**Act 6**

33. [P] Step 2, "Maya opens it by its alias": "alias" is undefined.
34. [P+E] What is labeled, "the rehearsal script discards the response in the browser": which script? The summary names three operator tools and this is none of them (record 5.2, 6.3 and 7.2 use it; appendix A does not define it).
35. [P+E] Diagram 03, "assembles S07's packet": S07 cannot be resolved from the edition (record front matter: a fixture of `SCENARIOS.md`).
36. [P] Diagram 03: F4 (session store to workspace server) and F13 (workspace server to the recorder) run along one straight line through the Session store box, so I could not tell which arrow leaves DS4; F2's and F11's labels sit on the B1 and B2 edges, F8's on B4's title.
37. [E] Diagram 03, F12, "[file export, the operator's profile ...]": the export is the recording browser's, which runs an actor's profile; I could not tell whose profile is meant.

**Chapter 7**

38. [P] "an assistant (MCP) scene and a public page": MCP is not expanded; "a public page" is not explained.
39. [P+E] Reduction 8, "declared internal commands, apart from external effects": I could not connect this seam to a mechanism.
40. [E] "the operator's just-in-time session" and "a write-once anchor": neither is defined (record 10.5: the operator role taken for one take; 6.7: an Object Lock anchor).
41. [E] Operations step 1, "Snapshot the database once ...; a retake reuses it": how the snapshot relates to the saved states `s0` to `s5` is not said (record 6.6 and 8: it is the recovery point, not a take's start).
42. [E] Operations step 2: there is no `s1`, and no reason is given (record appendix A: "act 1 changing nothing a take restores").
43. [P+E] Operations step 3, "inside the idle hour": undefined (record 10.6: a session idles out after an hour).
44. [E] Operations step 4, "the rate limit": first mention; whose? (Record 11.1: fAuth's raised login limit.)
45. [P] Cost, "tokens for 120 generation attempts": no basis (record 11.2: two attempts each for acts 3 and 6 over twenty takes, plus the spike and a margin).
46. [P] Diagram 04: F16's label sits across the operator's machine boundary; in diagram 02 the B2 and B6 titles are crossed by arrows. Both are readable only zoomed.

**Chapter 8**

47. [E] "until the restore drill times it": the drill is never described (record 11.4).
48. [E] "What to do first: the spike, then the labels.": I could not tell what work "the labels" is.

## 2. What surprised me or seemed inconsistent

Most consequential first.

1. **Seeded starts and the continuity of the six acts.** Acts 4 to 6 each say "The take starts from `s3`" (`s4`, `s5`), "labeled seeded"; the summary says a take is "one recording of an act or of the whole". The edition never says which the submission is. If it is per-act takes, act 4 publishes the revision 5 of an earlier clean act 3, not the one the partner just watched the model propose, and act 3's "what the reviewer approves in act 4 is what runs" holds only across takes.
2. **A labeled step not labeled where it appears.** Act 6, step 4 ("The retry") reads as the browser's own logic: it "asks for the outcome by key and re-submits `ik-1` unchanged only if none exists". The simulated part, the script forcing the re-submission, is labeled only further down, under "What is labeled"; the record's 7.2 labels it inside the step. Likewise acts 1 and 2 say "Nothing is simulated" and act 3 names only the spike's fixture, yet operations step 2 resets acts 1 and 2 to `s0` and act 3 to `s2`, and chapter 7 says the seeded state applies to "every take". Only acts 4 to 6 name their seeded start.
3. **Steps that look staged and carry no label.** The epoch-11 "late answer" (act 4, step 3), "A Finance query left running" (act 6, step 7), the stale edit "at expected version 0" and "`ik-1` reused with another payload" (act 6, step 5) each need a deliberately slow or hand-made request: a browser that has just received version 1 would not send expected version 0, and never reuses a key with a new payload. If the rehearsal script produces them, they are simulated steps; if not, the edition does not say what does.
4. **Act 5 claims a conflict it never shows.** Its trade-off: "act 5 shows a conflict, not a shared cursor". Act 5's steps contain no conflict, only "If Jordan and Maya edit at once"; the one conflict on camera is act 6's stale edit. The record carries the same split (3.2 and 6.3 against 7.1).
5. **The matched figures against a six-row file.** Act 2 imports "a six-row CSV"; act 4 has five events "all to `198.51.100.7`" split 240,000 matched and 900,000 unmatched; act 5 gives `ops-runner-04`, at most one remaining row, "210,000 bytes, 30,000 unmatched". The totals add up (240,000 + 900,000 = 1,140,000; 210,000 / 10 = 21,000), but I could not reconcile the splits with any per-event match (section 1, items 26 and 31).
6. **Diagram 01 against act 4.** Sarah's box: "Approves the exact draft; her own publish of it is refused". Act 4: Maya's "own approval is refused with 403", and Sarah approves.
7. **Diagram 01 against chapter 7, on the operator.** The operator's arrow is "[HTTPS to the API, platform key]"; chapter 7 makes the platform key fAuth's tenant key and has the administration API called "under the operator's just-in-time session" (diagram 04: F14 against F15). The operator's box says "reads no organization data"; chapter 7 puts "the raw browser export" on the operator's machine, the packet built there holds receipts and record outcomes, and the saved states the script captures hold "both organizations' rows".
8. **"nothing returns to the machine but identifiers and digests"** (chapter 7) against diagram 04's F13, "audit events, job states", and diagram 03's F12, outcomes and receipts classed Confidential, both arriving at the evidence recorder on the operator's machine.
9. **"The operator's shell holds four keys"**, but the list includes "fAuth's management keys", plural, and diagram 04's F16 writes the saved states to S3 under "SigV4", an AWS credential that is none of the four. Deploying the task needs AWS access too; the edition never counts it.
10. **"no network except during a take"** (chapter 7) against operations steps 2, 4 and 6: the reset, the dry sign-in, the canary suite and the evidence recorder all run between recordings, and step 2 says the reset is "never during a take". Either "take" means the six-step procedure in one sentence and the recording in the next, or the machine is on the network far more than the guard says.
11. **"the platform's nine component kinds as shipped"** (act 3) against the summary's honest part, four kinds rendered, and chapter 8's open "whether nine kinds fit in the 426 bytes left". "As shipped" reads as built. And "If the route is over, a kind is cut" seems too weak: five kinds in 426 bytes is about 85 bytes each, so more than one cut looks likely, and the edition does not say which kinds acts 3 and 6 need.
12. **"measured, not promised"** (two pictures) and "the saved work reopening within 30 s" (act 4), with nothing built: the restart maximum is one of the spike's three unknowns (chapter 8). I first read both as results.
13. **Two claims that seem too strong.** Act 3: "is evidence, not instruction: it reaches the model as a delimited data value". Delimiting does not stop a model being steered; what holds is admission against a closed vocabulary (the record's appendix C, TH11, says the field "can steer the proposal"). Chapter 7's accepted risks: "the canary suite bounds the rest". A test detects an isolation bug; it does not bound a compromised process.
14. **The canary: person, organization or script, and on camera or not.** Diagram 01 draws it as a [Person]; the summary calls it "a second synthetic organization"; chapter 7 "a second synthetic organization's scripted suite". Act 6, step 8 reads as an on-screen step. The edition never says whether the partner will see it (record 13.2: in the packet, not on camera).
15. **What act 5 revokes.** Jordan "is shut out within 5 s when his grant is revoked", and the revocation "moves the membership epoch", yet act 6, step 8 has "Jordan, in Security only". It reads as if his work grant went and his membership stayed, so "shut out" overstates it.
16. **"Three steps are labeled"** (summary, chapter 7), but the first, the spike's fixture proposal, never appears in a take, so the recording holds two. And "seeded, simulated or manual" names a category, manual, with no member, though the operator's revocation, the restart and the hand-added client row are manual.
17. **"eight reductions"** against act 5's "administration pages are a reduction" and diagram 02's "[reduced: the platform's edge]": neither is on chapter 7's list under that name.
18. **When the packet is anchored.** Operations step 6 anchors its digest after every take; the packet paragraph writes it "before delivery". Diagram 04 draws no arrow out of the evidence recorder for that write (record 7.3: it "rides F13's session back").
19. **Two lists of one-way decisions.** The front matter keeps "the platform's one-way decisions D1 to D4"; chapter 8 says "Three doors are one-way" and names a different three.
20. **Words that change meaning between chapters.** "evidence" (Maya's files; the demo's proof), "host" (the server; `ops-runner-04`), "labeled" (provenance, an export, the honesty label), "commit" (code in act 6's "zero commits"; a record in "commits version 1"), "revision" (file, draft, record, work), "role" (the four roles, database roles, the task role, the operator role).
21. **Small ones.** `ik-3` is minted (act 6, step 5) before `ik-2` (step 6). The summary's apps are "formed, published and reused", but only the first is reused. The front matter's "Read first: `design.md`" asks a twenty-minute reader to read 20,700 words first.

## 3. The partner's five questions, in order

1. **"Is the recording one take, or per-act takes from seeded states, and is the app Sarah approves the one I watched being formed?"** The edition does not say (section 2, item 1); everything else the partner concludes depends on it.
2. **"The model is called live: how do the same numbers appear in every take, and how many valid but different proposals did you throw away?"** The retake rules cover a provider failure (act 3; operations step 5), not a successful proposal that differs; the rehearsal count requires zero "platform-caused retakes in the last three" (chapter 8), which a discarded proposal may not count as.
3. **"In act 3, what came from the model and what was already there?"** Three of six views are "retrieved" in Northstar's first app, from a source the edition does not name; act 6 does not say how its own views are labeled, so I cannot tell whether Spend Review could have been retrieved too.
4. **"What happens if I type my own request, or ask for something the vocabulary lacks, and will even the nine kinds fit?"** The edition answers half: a kind asked for on the spot "is a release, not a prompt" (act 3), and "what a partner would test unscripted is unknown" (chapter 7); the 426 bytes for five kinds (chapter 8) is the question behind both apps.
5. **"Who is at the keyboard, and what is scripted?"** Chapter 7 puts "every actor's session" on one machine, act 6 has a rehearsal script in the browser, the record's 5.2 says that script has "one actor act at a time", and the canary's place in the video is unstated. Are Maya, Sarah and Jordan one founder in three Chrome profiles, and is the "independent review" two accounts or two people?

## 4. The engineer's five blockers

1. **What exists and what I write.** The summary says the demo "runs the platform's own code" and, in the same breath, "nothing of the demo's own is prototyped"; I cannot tell whether the workspace server, control plane, data service and workers are written, or whether the demo is their first build. Record: partly. Section 2 ("What exists") and section 5 ("Prototyped") list Forma 3.0.0, fAuth and the four-kind fixture; the front matter's K3 assumes "the stack tickets" done. Nothing says whether the platform's server code exists, and nothing gives a build order past the spike or an estimate.
2. **The fixtures and the matching rule.** The six event rows, the two inventory rows, the three expenses, and the rule that makes 240,000 bytes matched and 900,000 unmatched. Record: the totals only (4.2, N6) and "exact entry first" (5; 7.1); the rows live in `SCENARIOS.md` (S02, S04, S05), outside both documents. Nothing defines what matching means.
3. **The rehearsal script.** What it is (browser automation, an extension, a proxy), what it drives (one actor at a time and the dropped response; perhaps the late answers and the stale edit), and how it fits "a fresh Chrome profile per actor with sync and extensions off" and no developer tools on camera. Record: used in 5.2, 6.3 and 7.2, defined nowhere; it is not P9, P10 or P11 and not in appendix A. Nothing answers it.
4. **The authoring path's mechanics.** How two patches are made with one model call; what "Northstar's index" holds at `s2` so that three views come back retrieved; what a take accepts when a valid live proposal differs from the one the fixture numbers assume. Record: 4.1 (R3) requires the labels and patches and 7.1 restates the counts; nothing gives the mechanism. It may be in the platform record, which this check did not open.
5. **The saved states and the operator's credentials.** How `s2` to `s5` are produced, in what format, and with which AWS credential the script writes them. Record: mostly. Section 8 ("captured by P9 at a clean act's end") and 11.1 ("a clean take writing `s0` and `s2` to `s5`"), 6.7 and 9 (the reset's steps and guard), appendix A (why no `s1`). The manifest format is named as a cost (6.7) and never specified, and the AWS credential appears only as "a person with the AWS role" (10.3, TA12), not among AS12's four keys. 6.7 also says the script reads "no organization data" while it captures both organizations' rows.

Not a blocker: the spike. Its path, thresholds and fallback are in record 4.1 (R11), 5.2, 6.5 and 13.1 (RK1).

### The map, row by row (engineer pass)

| Chapter | Record sections named | Does the record hold what the chapter promised? |
|---|---|---|
| Summary in plain words | 1; 2; 3.2; 5; 11.2 | Yes. 3.2 (non-goals) feeds chapter 7, not the summary; the three labeled steps the summary names are in 13.3 and appendix A, which the row does not cite. |
| The system in two pictures | 2; 5; 5.1; 5.2 | Yes, and 5.2 keeps the bases the edition dropped: two operations an organization at once, the queue of 20 and its 429, 100 ms an operation. |
| 1. Act 1 | 5; 6.1; 7.1; 10.5; 11.1; 12.2; 12.3 | Yes. The client row and the public PKCE client are in 6.7 and 5.3 (D9), not cited; the 44 smoke tests are in 2. |
| 2. Act 2 | 3.3; 6.4; 7.1; 11.4; appendix C | Yes. The sandbox's memory cap (2 GiB) is in 11.2 and the oversize file's 422 in 9, neither cited; 9's 422 and the edition's "the sandbox dies" describe the refusal differently. |
| 3. Act 3 | 5; 6.2; 6.4; 6.5; 7.1; 13.1; appendix C | Yes for what the chapter says; the denominator argument is section 2, not cited. Where retrieved views come from and how patches are written is in no cited section, nor anywhere else. |
| 4. Act 4 | 5; 5.2; 6.2; 6.5; 7.1; 11.2; 11.3 | Yes; every claim found. |
| 5. Act 5 | 3.2; 6.1; 6.3; 7.1; 9 | Yes, but the chapter's own pointer "(sections 6.1, 11.3)" cites 11.3, which the row omits, and the 5 s canvas refresh is in 5.2. The record shares the act 5 conflict inconsistency (3.2 and 6.3 against 7.1). |
| 6. Act 6 | 4.2; 6.3; 7.2; 8; 9 | Yes; 7.2 labels the forced re-submission inside step 4, where the edition does not. The deploy-log check is in 6.7, not cited. |
| 7. What the demo does not prove | 3.2; 6.7; 6.8; 7.3; 10.4; 10.6; 10.7; 11.1; 11.2 | Partly. The machine's guards (fresh profiles, encrypted disk, the network) are in appendix C (TH15), not 10.4; the three labeled steps are in 13.3 and appendix A; the model and unscripted unknowns are in 13.1; the saved states' contents and seal are in 8. None is cited. |
| 8. How it grows, and what we do not know | 5.3; 11.1; 12.2; 13.1; 13.2 | Yes, but 5.3's decision table has no D2 or D3 row (only its closing line names them; 12.2 argues them), and the five migration steps themselves are the platform's 11.1. |

The platform-record column was not tested: this check allowed only `design.md`.

## 5. Would the partner take the next meeting, and could the engineer start

**The partner: yes to a next meeting, as a bet on the team rather than because the edition shows the demo proves its claim.** The claim is sharp and checkable: two different apps through one governed path on one build digest, every number with a receipt, the canary denied. The edition names what is seeded and simulated, and the packet gives a partner something to verify beyond the video. Against it: the status is Proposed and nothing of the demo is built, so "beside the recording" is premature; the edition never says whether the recording is one take; act 3's retrieved views have no stated source; the matched figures cannot be reconciled with a six-row file; the canary's place in the video is unstated. What would decide it: (1) one whole take of acts 1 to 6 with its packet; (2) the spike's route number, nine kinds against 28,100 bytes; (3) a rehearsal log showing the live model reproducing the fixture numbers across the twenty takes, every discarded take counted; (4) one sentence on where retrieved views come from. With 1 and 3 the next meeting is about the market; without them it is about whether the video is staged.

**The engineer: can start the spike on Monday, not the acts.** The spike is specified: its path (definition, query, receipt, command, outcome, reopen, the duplicate submission, the version conflict), its thresholds (28,100 bytes gzip; a commit at 300 ms median and 1 s at the 99th percentile; a restart within 30 s) and its fallback (cut a kind), in record 4.1 (R11), 5.2, 6.5 and 13.1. The cold start and provisioning are specified too (record 6.7, 11.1). The acts wait on section 4's blockers: what exists, the fixtures and matching rule, the rehearsal script, the authoring mechanics, the states' format and the AWS credential. What would decide it: a one-page list of what is built, what is assumed and what is the engineer's to write, and the fixture file with its matching rule; the rest can follow the spike.

## 6. Length

At 250 words a minute, with about two minutes a diagram (02 and 03 nearer three), the edition takes about 34 minutes. The partner's twenty end in act 6, at diagram 03, before the chapter that says what the demo does not prove.

| Chapter | Words | Minutes | Too long, too short or right |
|---|---|---|---|
| Front matter | 209 | 1 | Right; drop "Read first: `design.md`" for this reader. |
| Summary in plain words | 375 | 1.5 | Right. |
| The system in two pictures | 477 and two diagrams | 6.5 | Long for the partner: "Does it fit?" (about 130 words) and diagram 02's fourteen boxes are the engineer's. |
| 1. Act 1 | 410 | 1.5 | Too long for ten quiet seconds: the `oauth_clients` row, the fallback login and the dry sign-in are operator detail. Half would do. |
| 2. Act 2 | 350 | 1.5 | Right. |
| 3. Act 3 | 549 | 2 | Right for the core act; the 426-byte check belongs in chapter 8. |
| 4. Act 4 | 398 | 1.5 | Right. |
| 5. Act 5 | 310 | 1 | Short: it lacks the conflict it claims and what the revocation removes. |
| 6. Act 6 | 706 and diagram 03 | 6 | Long for the partner: eight engineer-level steps and a dense diagram; steps 2, 3, 4 and 8 carry the proof. |
| 7. What the demo does not prove | 1,316 and diagram 04 | 7.5 | Too long and too late. "Security in brief" (425), "Operations in brief" (330) and the accepted risks (175), with diagram 04, are the engineer's; "What it does not prove" (74) and the three labeled steps (74) are what the partner needs, and they arrive after minute 22. The first of those is short for its weight: it should carry section 3's questions 1, 2 and 5. |
| 8. How it grows | 584 | 2.5 | Right; "How it grows" (182) is the engineer's. |
| 9. The map | 263, a table | 2 | Right for the engineer; the partner skips it. |

To fit twenty minutes: move chapter 7's security, operations, cost and accepted risks (about 930 words and diagram 04), "Does it fit?" and diagram 02 into an engineer's part after the map; halve act 1; condense act 6's steps 1, 5 and 7; and put "What it does not prove" and the labeled steps straight after the summary. That leaves about 4,200 words with diagrams 01 and 03, about twenty-one minutes.
