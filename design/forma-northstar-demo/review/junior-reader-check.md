# Junior-reader check: the annotated edition of forma-northstar-demo

**Who I am.** A developer with two years' experience who will build the Northstar demo next. I
have read one design document before, the platform's annotated edition, and did not have it open
for this one.

**What I read.** `design.md` and `annotations.md`, in full and side by side. The built page
`output/design-annotated.html`, read through its HTML and the hover tables it carries (the
identifier definitions and the glossary terms) rather than clicked in a browser: the Start here,
Document, Course, Glossary and Exercises tabs, every note and every Try-it box. And
`reference/TEACHING.md`, whose section notes and eight lessons the page shows. Nothing else.

**The short version.** What helped: the notes that explain a mechanism instead of restating a
sentence. The best were on the record command (8, 9, 7.2), the canary's paired denial (R8, SR2),
five database roles in one process (5.2, D1, 10.5) and the reduction test (2), along with
exercises that make me compute (4.2, 5.2, 11.2). What did not help:

- The page leaves out the record's reading note, and the very first note sends me to it.
- The record's own glossary (appendix A) never reaches the hover or the Glossary tab, although
  the appendix A note says it does. So spike, reset, packet and rehearsal flag have no definition
  where I first meet them.
- Identifier links mislead in several regular places, and the first note's warning names a form
  that is no longer linked.
- Nine notes sit away from their keys.
- "Take" has two sizes, and the numbers mix them.
- Three exercise answers lean on sources outside the record.

## 1. What I still could not follow after the notes

- **Before 1 (page).** The page leaves out the record's "How to read this record" paragraph and
  runs from the cover table straight into 1. The first note opens "The reading note above the
  summary sorts every identifier into two kinds", and 3.3 says "provenance: the reading note".
  There is nothing there to read. So on the page the identifier families, the status tags, where
  A1 to A14 come from and the names `ik-1` to `ik-3` are never introduced. I found the paragraph
  only in `design.md`.
- **1, the first note's pitfall.** It warns that hovering "platform D5" shows this record's D5.
  On the page no lowercase "platform X" is a link at all: there are 116 of them, all plain text.
  The links that do mislead are forms the note never mentions. Each shows this record's
  identifier:
  - "the platform's A4" (A6)
  - "Platform D1 to D14" (A11)
  - "Platform SR1, SR2, SR4" and "SR3's service token" (4.2, 10.7)
  - "the platform's AR2" and "AR3" (10.6, 10.7)
- **1.** "adds three operator tools, the reset under a rehearsal flag" names one tool of three.
  No note names the other two, the provisioning script and the evidence recorder, before
  section 6.
- **2 and 4.1, the measure.** The 2 note says "the denominator lives in code", and the task map
  calls the whole-window denominator the meaningful change. Nothing says what it replaces, so I
  could not see why the patch changes a calculation rather than restyling a view. Nothing says
  either why 2 counts five events where 7.1 has six of `build-worker-17`; I guessed the half-open
  window drops one.
- **"Take" (3.1's G6 note, 4.2, 11.2, 11.3, appendix A).** The note and the glossary define a
  take as "one recording of an act or of the whole demo". They do not say which size each number
  uses:
  - 4.2 has "ten rehearsals of up to two takes, twenty takes" inside a 30-minute rehearsal of
    about 200 refreshes.
  - 11.3 has "a take's 200 refreshes".
  - N2 has "one restart a take, the maximum over twenty".
  - 11.2 has two generations "for acts 3 and 6 a take".
  - Appendix A says a take is "one recording of an act".

  I cannot tell whether the rehearsal script and the evidence recorder count takes per act or per
  run.
- **Labels (3.1's G6 note and appendix A).** The note teaches three labels: seeded, simulated and
  manual. Appendix A's "Labeled step" ends "are the three", meaning three particular steps, and
  13.3 lists more than three labeled steps. On first reading I took "the three" to mean the three
  labels.
- **5.3's last column and 6.8's seam column.** The 5.3 note says to read the last column as the
  platform's decisions. The page, though, links those bare numbers to this record's. In D4's row
  "D6" shows provisioning by script, and in D5's row "D10, D1" shows the ECS task and one process.
  6.8's "D3, D4", "D8", "D2" and "D5" do the same, and its D11, D12 and D14 are defined nowhere in
  this record. No note in 6.8 warns.
- **6.1 to 6.4, the component ids.** Hovering P2 shows a 10.2 entry-point row ("P2 to P4; P2 to
  P3 at admission"), and P9 shows an 11.4 failure row; neither shows the component. P3, P5 and P6
  have no hover. P7 is never named, though it appears in 6.4's heading and in 7's "P2 to P5 and
  P7".
- **6.2.** The example puts "the denominator patch" after Sarah's approval, and the note repeats
  that it "voids her approval". But 7.1 makes the denominator act 3's revision 2, patched before
  any approval. Nothing says the 6.2 case is R4's separate test ("a later change invalidates the
  approval (a test)").
- **6.4, 7.1, 11.2 and TH10, the parser's cap.** 7.1 says that when the 4 MiB file inflates past
  50 MiB, "P6 dies at its cap". The 11.2 note says the module "may use up to 2 GiB before the
  runtime stops it". I cannot tell which limit kills the module and which only refuses the file.
  Exercise 7's answer assumes the 50 MiB cap kills it.
- **6, against Lesson 4.** The course asks for an extension point in the form "to add Y you add
  Z without editing W". The demo's cards have none, and no note says the seams are the place to
  practise this, though 6.1's seam is written in exactly that form.
- **7.2, against Lesson 5.** The lesson's activity begins "Take the sequence diagram". This record
  has none of its own; it says platform 7.3's diagram holds "message for message". No note offers
  7.2's numbered steps as a stand-in.
- **9, against its method note.** The method note says to look for an authentication column.
  Section 9's table has none, because authentication per entry point lives in 10.2's table, and
  no note links the two.
- **10.2 to 10.4, the platform's families.** SI1 to SI6, AS1 to AS8, TL0 to TL8 and TA1, TA3,
  TA5 and TA6 are the platform's and are not defined here. The rows still depend on them, as in
  TH4's "SI1" and "negatively impacting AS3, AS4". So Lesson 6's trace from a row to its
  invariant cannot be finished for TH3, TH4, TH5 or TH7 from this record.
- **10.3, TA.** The first note says three families continue the platform's numbers: TL, SI and
  AS. TA does too, since TA11 follows the platform's TA1 to TA9. The platform's TA2, TA3 and TA9
  are cited bare.
- **10.4, the paragraph after the table.** "Re-rated: TH8, … TH11, … TH14" names the platform's
  rows without the prefix; appendix C does prefix them. The page links them to this record's TH8
  (the reset racing a command), TH11 (the injected field) and TH14 (a deploy between acts). The C
  note's advice to read "platform TH" with its prefix cannot help where the prefix is missing.
  M29 is defined only inside this sentence and in appendix C's prose, never in a Mitigation cell.
- **10.5 and appendix C, more bare platform ids.** In 10.5, "(F7; SA2)" links to this record's
  SA2, fAuth's client row, which has nothing to do with re-reading membership. In appendix C,
  "M16's service token" collides with this record's M16, the pre-take row count. Neither is
  flagged.
- **12.1 and 12.2 (page).** The notes keyed to D1, D2, D3, D4 and D7 sit at the top of 12.1,
  before 12.1's own paragraph and before the 12.2 records they explain. So I read "Option B is
  the trap this design is one setting away from" before I knew what option B was.
- **Appendix A, its note.** It says both glossaries "are merged on the Glossary tab". The tab
  holds only the 211 annotation terms. None of appendix A's seventeen entries has a tab entry or
  a hover under its own name, among them spike, packet, reset, pre-take assertion, fixture state,
  rehearsal flag and supervisor. The only exceptions are take and rehearsal, which the layer
  defines again.
- **Appendix B (page).** The notes keyed to TH9, TH10, TH14 and TH15 land in appendix B, above
  the reference list, not beside their rows in C.
- **Hover false matches (page).**
  - "design as", in the first sentence of both 1 and 5, links to "design A", the earlier record.
  - "rehearsal" in "rehearsal flag" and "rehearsal script" shows what a rehearsal is.
  - "an unsanitised export" (6.7) shows the findings export.
  - "S3", the AWS service, is linked as an identifier to 11.2's cost row.
- **Start here.** The task map's "Where the record answers it" cells are plain text, so every
  pointer has to be found by hand.

## 2. Words used before they were defined

| Word | First appears | Defined, if anywhere |
|---|---|---|
| spike | 1: "the spike, the rehearsal procedure and the packet" | In passing in the G6 note (3.1); in 5 and appendix A. No hover. |
| packet | 1: "once the packet is submitted" | The reading note, which the page omits; the 2 note in passing; appendix A. No hover. |
| reset | 1: "the reset under a rehearsal flag" | The G6 note in passing; the opening of 6 and 6.7; appendix A. No hover. |
| rehearsal flag | 1 | The opening of 6, 11.1 and appendix A. The hover on "rehearsal" defines a rehearsal instead. |
| [built], [proposed], [reduced: …] | 5: "The [built] pins are" | The reading note only. The 2 note explains built and proposed in words; nothing explains the bracket form. |
| whole-window denominator | 4.1, R3 | Nowhere. |
| real-data disclosure | 4.2, N8: "the composer's real-data disclosure stays off" | Nowhere. |
| M1 to M31 | 4.2, SR1 to SR6 | The Mitigation cells of 10.4 and appendix C; M29 only in prose after 10.4's table. No hover. The SR2 note glosses M29. |
| pre-take assertion | 6.1 (the D6 note in 5.3 uses it first) | 11.1 step 4 and appendix A. No hover. |
| trusted base | 6.3: "the DuckDB engine in the trusted base" | 10.2 and its note, as "trusted computing base". The hover matches only the long form. |
| explanation, as a job | 6.4: "indexing and explanation as jobs" | Nowhere. Its only other use is 11.2's "no explanations (N8)". |
| P7 | 6.4's heading | Nowhere in this record. |
| application budget | 6.5: "FORMA-232's application budget" | Nowhere. The D5 note repeats the phrase. |
| `createResource` | 6.5 | Nowhere. |
| fixture state; `s0`, `s2` to `s5` | 6.7 | 8's retention paragraph and appendix A; the memento note glosses it. No hover. |
| SI1 to SI6 | 10.2 | The platform record only. |
| AS1 to AS8; TA1, TA3, TA5, TA6; TL0 to TL8 | 10.2 to 10.4 | The platform record only; some rows describe the actor in words. |
| L2; phase 7 | The cover ("at phase 7 (L2)"); 2 ("its phase 7 gate commit") | Nowhere on the page. Start here names the phases, but not their numbers or the levels. |
| the editions | 3.3, A1: "If wrong: the editions" | Nowhere. |

## 3. The exercises

- **After 1, the three one-way doors.**
  - *How I could answer.* By derivation from the summary, as the question asks ("Before reading
    5.3"). After 5.3 it becomes lookup, because the 5.3 note "Why only three doors are one-way"
    states all three reversals.
  - *The given answer.* Right, and backed by 12.2 ("keys cannot be retrofitted on camera"; "a
    packet cannot be relabeled after YC has it") and by 5.3's two-way label on D9. One gap: the
    question asks whose action each reversal needs, and the answer says so outright only for D7.
- **After 2, a fourth shortcut for act 5.**
  - *How I could answer.* By derivation; the answer is open.
  - *The given answer.* Right, and backed by G3, R6, act 5's "Captured" line in 5, and 7.1 act 5.
    Two problems with the wording. Its second version, filming Maya's tab under Jordan's name, is
    section 2's third shortcut again, not a fourth one. And "two work ids where act 5 must show
    Maya's one id reopened and Jordan's own new one" is hard to parse, because a correct act 5
    also files two work ids.
- **After 3.3, the three sign-in mornings.**
  - *How I could answer.* (a) and (c) by lookup: A12's and A13's "If wrong" clauses and D4's
    row. (b) only by common sense from the sources the question allows.
  - *The given answer.* Right, but (b) rests on SR3, D9's row and 11.4's provisioning row, none
    of which a reader at 3.3 has reached. (a)'s "before the take, never during it" and "sessions
    live 12 h" also come from later sections (11.1, 6.9), not from the only sources the question
    names.
- **After 4.2, 360, 1,200 and the samples above the p99.**
  - *How I could answer.* By derivation.
  - *The given answer.* The arithmetic is right and backed by 4.2 and 11.3: 1,800 s ÷ 5 s is 360;
    200 × 6 is 1,200; 100 edits and uploads remain; 4 and 2 samples lie above the pooled p99s.
    But the question's "one take's 200 refreshes" follows 11.3, while 4.2, the section the
    exercise is attached to, puts about 200 refreshes in a whole rehearsal of up to two takes. I
    could not tell whether a take or a rehearsal holds the 200.
- **After 5.2, the burst.**
  - *How I could answer.* By derivation.
  - *The given answer.* Right, and backed by 5.2 and 6.3. One actor: 6 operations, 2 running and
    4 waiting, three waves, 0.3 s. Two actors with eight views: 16 operations, 14 waiting, no
    429, eight waves, 0.8 s, and a ceiling at 250 ms. One wording slip: 5.2 puts the 429s past 20
    waiting, while the answer says "the 20 waiting at which 429s begin".
- **After 5.2, the grace and the restart drill.**
  - *How I could answer.* (a) and (b) by derivation. (c) by lookup: 6.7 says "a task start on
    the same image digest, act 4's restart drill, is not a deploy", and TH14's M28 says how the
    recorder tells a restart from a deploy.
  - *The given answer.* Right; (a) checks out, with the third failure 20 to 30 s after the crash.
    (b)'s "the task replacement the supervisor exists to avoid may follow" is an inference the
    record implies but never states.
- **After 6.4, the 9 MiB file and the 4 MiB file.**
  - *How I could answer.* The first half by derivation, from A6's 5 MiB cap and from appendix C's
    M19 ("the byte caps refuse before parsing"), which a reader at 6.4 has not reached. The
    second half by lookup in 7.1's act 2 and 11.4's parser row.
  - *The given answer.* Right on the byte cap and on the platform's 10 MiB, which A6's "half for
    XLSX" gives. Not settled by the sections it names: the answer has the module die "at its cap"
    at 50 MiB, while the 11.2 note has the runtime stop it at 2 GiB.
- **After 6.7, deleting sessions instead.**
  - *How I could answer.* By derivation.
  - *The given answer.* Right, and backed by 11.1 step 3 (which relies on the reset keeping
    sessions), 11.4's limiter row, SI7's "never deletes audit or sessions" (10.2), and TH9.
- **After 7.2, tracing `ik-1`.**
  - *How I could answer.* The trace by lookup, from 7.2 steps 3 and 4 nearly word for word. The
    comparison of the two recoveries by derivation.
  - *The given answer.* Right, and backed by 7.2 and 9. Its "S05's rule that an attempt log is
    not a second mutation" points at the fixture; the record's own words are 7.2's "the attempt
    logged apart".
- **After 8, why there is no `s1`.**
  - *How I could answer.* Mostly by derivation. Act 1's leftovers are lookup (7.1's "one session,
    one audit row"), and so are the two checks (TH2's M3 and M4).
  - *The given answer.* Right, with one citation slip. "The reset's audit row marks the
    restoration itself as seeded" is cited to G6 and 7.2, but the sentence is section 9's ("enters
    the rehearsal log as seeded state") and appendix A's, not 7.2's.
- **After 10.4, the paired canary.**
  - *How I could answer.* The row by lookup: M8 is TH4's detective half. The two broken setups by
    derivation.
  - *The given answer.* Right, except for "SI1, the platform's scope invariant", which is not
    backed: this record only names SI1, so I could not check the gloss.
- **After 10.6, the three leaks.**
  - *How I could answer.* By derivation, with lookups in D9, 6.7, 10.6, M30, AS13, TH15 and AR3.
  - *The given answer.* The facts are right and backed. The ranking the question asks for is not
    given cleanly, though. (a) is called "Least", then (b) is "Nothing, if the rule was kept", so
    I could not tell whether the order is a, b, c or b, a, c.
- **After 11.1, the record schema on the run path.**
  - *How I could answer.* The first part by lookup (7.2: P2 reads the record schema from the
    publication row, F5). The prediction by derivation, from what 11.1 says step 4 needs.
  - *The given answer.* Right. Its claim that the control plane can be down while published apps
    serve is cited to platform 11.4, which cannot be checked from this record.
- **After 11.2, the 8 GiB and the month.**
  - *How I could answer.* By derivation.
  - *The given answer.* Right, and backed by 11.2:
    - 8 GiB; $59.10 + $25.96 = $85.06.
    - $168 for the month, $16.80 a rehearsal.
    - $0.092 an attempt, $11.04 in tokens.
    - At twenty rehearsals: 200 attempts, $18.40, about $175 a month and $8.80 a rehearsal.

    Fargate is the driver, and it does not move. The answer holds the other lines still, which is
    fair: logs and egress would add cents.
- **After 12.2, the founder's machine.**
  - *How I could answer.* By argument. Its second and third parts are lookup: 12.2's table row
    for D10, and 13.2's "kept if its timed replacement meets N2's 30 s, else one virtual machine".
  - *The given answer.* Right, and backed by 11.2, 12.2 and 13.2. The $157 is 168 − 11.
- **After 13.3, owner publication.**
  - *How I could answer.* By argument. The catch is lookup: 6.2's pre-take assertion and RK4.
  - *The given answer.* What the change breaks is right and backed by 6.2 and 7.2 step 2. Two
    parts are not backed by the record. The case for it quotes `PRODUCT-BRIEF.md` ("authorized
    owner publication for an appropriate low-risk scope"), a phrase the record never contains.
    And "which S03 checks separately" appears nowhere in the record.

**By lookup alone.** None of the sixteen as a whole. These parts are lookup:

- 3(a) and 3(c)
- 6(c)
- the trace in 9
- the first part of 13
- the second half of 7
- the row in 11
- the catch in 16
- all of 15 except one subtraction

## 4. Notes that repeat the record instead of explaining it

- **6.5, second paragraph.** "The other number in this card, 426 B" restates the card's own
  sentence and cites "(6.5)" inside the 6.5 note. It is also the third time the layer explains
  the 426 B, after the 2 note and the D5 note.
- **7.3, "No demo switch."** Restates 7.3's own lines ("no demo switch", "synthetic data only")
  and section 9's "denied by the platform's check, not a demo switch". It cites "(7.3; platform
  R16)" from inside the 7.3 note, and adds no reason the record lacks.
- **11.1, "Test the absence on every build."** A paraphrase of 11.1's rehearsal-flag paragraph.
  The TH1 note already teaches the same test, with its reason.
- **11.1, "Why act 1 starts signed out of fAuth too."** Restates step 3. Its one addition is the
  gloss on "the idle hour".
- **8, "What the reset keeps."** Restates 8's retention paragraph: audit, sessions, the
  operator's account, the epoch bump and the sealed digest. The 6.7 card and the TH9 note cover
  the same reset, so it gets three passes.
- **10.6, "Every control fails closed."** The middle sentence is the record's list, in the
  record's order. Only the last sentence explains.
- **TH15.** The parenthesis copies M30's list item by item. The framing around it (shrink what is
  on the machine, make tampering visible) is the useful part.
- **TH14.** Restates M27, M28 and 6.7's restart sentence, and exercise 6(c) then asks for the same
  facts.
- **6.4, "Never continue on the fixture."** The card already says "never a continuation on the
  fixture" and gives the G6 reason.
- **11 and 11.4.** Both say that the failure table's last column is what happens to the take and
  that the take is the runbook. 11.4's last sentence restates the record's runbook rule.
- **13.2.** Restates the current answers (the ECS task, the labeled fixture) and the "Still
  unknown" list. Its first sentence, that each question carries its current answer, is the
  lesson; the rest is the record.
- **13.3, last sentence.** Repeats the course's Lesson 8 activity rather than explaining the
  record ("a gap or a question only you would ask").

## 5. The three things the layer taught me best

1. **A denial is evidence only beside its owner's success** (the R8 and SR2 notes, the 10.4 note
   on TH4, exercise 11). A bare 404 cannot tell "refused" from "no such thing". Pairing it with
   the owner's 200 in the same take is what turns it into evidence. Running the suite at both
   ends of a take also covers the identifiers the take itself created. I now know how to write a
   negative test that cannot pass by being broken.
2. **The record command's order of checks** (the 8 note on the unique index, the 9 notes on
   retrieving before re-submitting and on 409 rather than 422, the 7.2 note on counting, exercise
   9). I will build the decision form from two sentences: "Creation is the one case a lock cannot
   serialise", and "a careful client, and a server that does not depend on the client being
   careful".
3. **Collapse deployments, not boundaries** (the 2 note on what a reduction is, the 5.2 note on
   five database roles in one process, the D1 note on option B, the 10.5 note on the typed
   principal). A reduction keeps the interface of what it shrinks. The four non-reductions
   replace a mechanism with its appearance. And per-role grants mean "The walls stop mistakes,
   not an attacker already inside". This is the test I will apply to every shortcut I am tempted
   by while building.
