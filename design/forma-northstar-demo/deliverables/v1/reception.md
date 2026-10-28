# Reception: forma-northstar-demo

The document read by the person who will actually receive it, before they do. The first part is
written by a fresh agent in the recipient's role, from the submission and the brief only. The
second part is the lead's triage: every question and objection answered, fixed or accepted, so
the head can anticipate the meeting. `npm run receive -- forma-northstar-demo` assembles the
recipient's brief; `npm run check` (M22) verifies the triage is complete.

## Who read it

- Recipient: a YC partner at Y Combinator, reading Forma's demo against the Fall 2026 request "A
  Cloud for Small Software". Decides whether the recording and this document earn Forma the next
  step. Weighs whether what is on screen is working software rather than a staged mock, whether it
  shows a platform rather than one app, what exists today against what is planned, whether one
  founder can ship it, and whether any team wants it.
- Received: design-demo.pdf, 19 pages, the demo's edition, status Proposed; with the brief, which
  is the request and the two product definitions it attaches (the Forma App Platform Product
  Definition, 25 pages, and the earlier Forma Platform Product Definition, 23 pages). No recording
  and no evidence packet came with it, though the document says the proof ships with the video.
- Read in: twenty minutes, with the six acts as the spine, after reading the brief. About five
  minutes went on the first pass: cover, summary, "what is real", the partner's first questions
  and the four diagrams. About thirteen went on acts 1 to 6 and chapters 7 and 8 (pp. 8 to 15),
  checked against the brief's six acts and its evidence-of-completeness list, with the page images
  for the diagram pages and the half-empty pages 6, 11 and 16. The last two went on skimming
  chapter 9 for the canary, the packet, the cost and the accepted risks. At this density pages 3
  to 15 do not fit twenty minutes comfortably.

## First impression

This is the plan for a demo video, not the video. Forma wants to show a fictional company,
Northstar, building a security investigation app from two small files. A second person approves
it and a colleague reuses it. Then a different app, a finance spend-review queue in another space,
is built the same way on the same build, with no deploy in between. Meanwhile a second fake
company replays Northstar's ids and gets nothing back. What stood out: the model proposes a typed
app definition that the platform checks, so it is data, not code the platform runs. And the
document is unusually straight about what is real, seeded, simulated or manual, down to "status
Proposed" and "designed, not built". What bothered me is what that candour reveals. Beyond the
framework, a prototype canvas and the sign-in service, nothing is built. There is no date and no
user. And much of the design goes to proving the recording untampered, for a reader like me who
will watch the video and never open the packet. The first picture reads at a glance; I couldn't
read the other three at page size.

## Against the brief

| Criterion | Verdict | Evidence | What would make it strong |
|---|---|---|---|
| The platform is the product and Northstar only the example (the request; definition sections 1, 16, 20) | strong | p. 3: the summary leads with what "a platform must" do, "two different apps formed and published through one path on one build"; p. 14: the design refuses "a hardcoded act 6" | nothing |
| Act 1: the company home first, with its permitted spaces, apps and recent work; slash and click equivalent (section 16, act 1) | weak | p. 8: a sign-in at fAuth, "ten quiet seconds", then the Security space "by click or /security"; no home, no app library, no recent work | Open on the home with both spaces and the library; show /security and a click landing on the same place; move PKCE and the tenant claims to chapter 9 |
| Act 2: data in, meanings confirmed, nothing published (section 16, act 2; section 8) | strong | p. 8: row counts, bytes, UTC, the half-open window and the match rule confirmed by Maya; "two file revisions, nothing published"; the inflated-spreadsheet refusal | Say whether the refusal is on camera |
| Act 3: find, adapt or generate and say which; a meaningful transformation; preview, accept, undo (section 16, act 3; section 9) | strong | p. 9: views marked retrieved, adapted or generated; the rate's ten-minute denominator set in the definition with a diff; the observation form; undo as revision 4 | Ask for one of the two transformations in words, so the model's part shows beyond the first call |
| Act 4: review that fits what changed, a known published version, run, save, return (section 16, act 4; section 12) | strong | p. 10: self-approval refused with 403; Sarah approves the exact digest; publication 1; the case pinned to it; reload and restart bring the work back | Show the app in the company library with its owner, version and required inputs |
| Act 5: shared work and independent reuse as distinct paths, access judged separately (section 16, act 5; section 11) | adequate | pp. 10 to 11: the case grant is apart from app access; a bare link answers 404; Jordan's work has its own id; revocation holds within 5 s, but it is done by the operator's script; Jordan's source looks like Maya's import (p. 8) | Revoke from Maya's share control in the product; have Jordan use a source he is separately allowed to read, and say whose it is |
| Act 6: a different app built the same way, in another space, with no custom platform code (section 16, act 6) | strong | pp. 11 to 13: Spend Review in Finance, with a queue, a detail view and a decision form; the same build digest; the deploy log silent between acts 3 and 6; back to Security with the work intact | Give the queue more than one item and move the retry and stale-edit steps to the packet, so the act is about the second app |
| An app beyond the supplied demo, with no platform code changed (section 18, evidence of completeness) | weak | p. 4 and p. 13: "what a partner would test unscripted is unknown"; p. 9: the registry carries the two demo apps' own operators | A third app named by someone else, built live or in a labeled unscripted take; say when the nine kinds were fixed |
| Fixture data and simulated behavior labeled wherever used (section 16, act 6; section 20) | adequate | p. 3 lists every seeded, simulated and manual step, and each act repeats its own; but the labels are said to live in the rehearsal log and the packet (pp. 15, 18), and p. 3 quotes a "demo's brief" phrase that is not in the brief I have | State that each such step carries an on-screen caption at the moment it happens |
| Trust shown rather than asserted: a denied operation, the boundary between companies, injected instructions (sections 2, 13, 17) | adequate | p. 10: the self-approval 403 and the bare-link 404 are on camera; but the canary is "in the packet, not on camera" (pp. 13, 15), and the injected log field leads to no visible refusal (p. 9) | Put one canary replay on camera: a member of the second company pastes a Northstar link and gets a 404 |
| The live product kept apart from the vision, with none of the claims the brief excludes (sections 2, 15, 20) | strong | p. 3: "designed, not built"; p. 13: "no customer data and no pilot"; p. 15: targets "not results"; p. 18: "the real month waits on the meter" | nothing |
| YC's request: small software as easy to share with colleagues as a Google Doc (section 20) | adequate | The request is never named; act 5 shares a case (p. 10), but described as grants, ids and epochs, and the revocation is scripted (p. 11); nowhere on screen do the company's apps live | Name the request once; make act 5's share and unshare two clicks in the product |
| A working product and customer evidence, which the brief says decide the case (sections 19, 20) | weak | p. 3: four parts designed, not built; p. 13: no pilot; p. 4: the founder runs every account | The recording, with a date; one team outside the founder using one app on its own data |

## Margin notes

Page by page, as the recipient would write them in the margin. Kind: praise, question, objection,
confusion, nit.

| Page | Section | Kind | Note |
|---|---|---|---|
| 1 | Cover | confusion | Status "Proposed", yet the subtitle has me "watching the recording". On the day, is there a video with this or not? |
| 1 | Cover | nit | "Read first" sends me to design.md and a repository path, and "as corrected after the phase 8 reader check" is your process. Neither is for me. |
| 3 | Preamble | nit | A citation key for two documents I don't have: "section 7.2", "platform 11.1", "platform D1", "a bare D number". From here on every pointer is a dead end for me. |
| 3 | Summary | praise | The first paragraph is the pitch I'd retell: two different apps, one path, one build, the first reused by a colleague. "Data the platform checks rather than code it trusts" is the line I'd repeat to the group. |
| 3 | Summary | praise | "The honest part": status Proposed, what is built named by its commit, the rest "designed, not built". That is rare, and it makes me trust the rest of the page. |
| 3 | Summary | question | The document never names the request it answers. I connected it to "A Cloud for Small Software" myself. Name it once, and say which act lands the Google Doc line. |
| 3 | Summary | question | The proof ships "with the video in a packet". I won't open a packet. Who is it for? |
| 3 | What is labeled | confusion | "The demo's brief asks ... 'excluded from working-capability acceptance'": that phrase is not in the brief I was handed, and "S07, the record's scenario" means nothing to me. |
| 3 | What is labeled | question | Labeled where: on screen when it happens, or in the rehearsal log (pp. 15, 18)? And "the rehearsal script drives the take", while p. 4 puts the founder at the keyboard. Who is clicking? |
| 4 | The partner's first questions | praise | These are my questions, each answered in a line. The best block in the document. |
| 4 | The partner's first questions | objection | "What a partner would test unscripted is unknown." That is the first thing I would ask to try, and the answer is "unknown". |
| 4 | How the recording is cut | question | Six takes joined, with saved states restored between them, is fine if labeled. How long is the finished video? Act 3 alone is "a five-minute act" (p. 9). |
| 4 | The system in two pictures | nit | "B for trust boundaries, F for flows, P for processes, A6 for the input caps", D1, D10: the engineer's legend, sitting in my part. |
| 5 | Diagram 01 | praise | The one picture I'd keep: four people on top, the demo as one box, the canary in its own organization, fAuth and the model provider below. It reads at a glance. |
| 6 | Text after diagram 01 | nit | Two paragraphs, then a blank page. Pages 11 and 16 do the same before each landscape diagram. |
| 7 | Diagram 02 | nit | The labels can't be read at page size and the arrows cross everywhere, while the caption is printed twice, small above and large below. The same goes for pp. 5, 12 and 17. I followed the text instead. |
| 8 | Act 1 | objection | Act 1 is a sign-in page and "ten quiet seconds". The brief's act 1 is the company home with its spaces, apps and recent work, so that the platform shows before any app. I never see the home. |
| 8 | Act 1 | question | Why your own identity service? fAuth costs a second deployment and a hand-added database row, and two of the five accepted risks come from its missing APIs (p. 19). "Why fAuth, section 12.2" is in a document I don't have. |
| 8 | Act 2 | praise | Bytes, UTC, the half-open window and "unmatched means absent from this inventory, not malicious" are confirmed before any number exists. That is the brief's point about data meaning, done properly. |
| 8 | Act 2 | question | Is the inflated-spreadsheet refusal on camera, or only in the packet? "What you see" doesn't include it. |
| 8 | Act 2 | nit | "Uploads a two-row inventory of destinations, a spreadsheet, from the keyboard": I reread sentences like this all through acts 1 to 6. At this density my part does not fit twenty minutes. |
| 9 | Act 3 | praise | Each view says whether it was retrieved, adapted or generated, and the per-minute denominator is set in the definition with a diff. That is the brief's own bytes-per-minute example, done right. |
| 9 | Act 3 | question | What did the model do? One call that arranges nine prebuilt kinds, and both transformations come from controls, "not by the model". Why not ask for the rate in words? |
| 9 | Act 3 | objection | The registry holds the nine kinds plus "the network-range lookup and the integer currency operators", exactly what your two demo apps need. Were the kinds fixed before or after you picked the apps? |
| 9 | Act 3 | question | The "export every secret" log field: do I see anything refused, or do I only read that admission would hold? The brief invites showing that denial. |
| 10 | Act 4 | praise | Maya's own approval is refused, Sarah approves the exact digest, and a later patch no longer matches. Separation of duties shown, not described. |
| 10 | Act 4 | question | A process restart as a scene, with up to 30 s to come back: is that dead air in my video? The reload already shows the work persists, and the late answer from epoch 11 is dropped with nothing to see. |
| 10 | Act 4 | question | The brief puts the published version "in the company library" with its owner and inputs clear. Where is the library? |
| 10 | Act 5 | praise | Case access is a grant apart from app access, a bare link answers 404, and Jordan's work has its own id and empty findings. The brief's two paths, kept apart. |
| 10 | Act 5 | question | Whose data does Jordan's ops-runner-04 work read? Page 8 puts both ops-runner-04 events in the CSV Maya imported, which would make it her upload, not his data. |
| 11 | Act 5 | objection | Maya grants access in the product, but the revocation comes from the operator's script through an admin API. If sharing is to be as easy as a Google Doc, unsharing is a button. |
| 11 | Act 6 | praise | "Made on camera, not loaded ... no deploy between acts 3 and 6; the evidence recorder fails acceptance if the deploy log shows one." The generality claim, made checkable. |
| 13 | Act 6 | objection | The act that carries generality spends four of its six steps on idempotency keys ik-1 to ik-3, a stale edit and a late answer, three of them simulated. The second app gets lost behind them. |
| 13 | Act 6 | nit | The queue holds one expense out of three. A queue of one does not read as a queue. |
| 13 | Act 6 | question | "Opens it by its alias, since the index may lag publication by up to 60 s": so the app she just published isn't where colleagues would look for it? |
| 13 | What it does not prove | praise | "There is no customer data and no pilot" and "what a partner would test unscripted is unknown", said plainly. |
| 13 | What it does not prove | objection | The canary is the isolation the summary says the demo optimises for (p. 3), yet it is "in the packet, not on camera" (and p. 15). The video never shows a boundary between companies. |
| 14 | The eight reductions | praise | "Four shortcuts would not be reductions, and the design refuses them: a login dropdown, work only in browser memory, faked record commits, a hardcoded act 6." I'd quote that back. |
| 14 | The one-way doors | confusion | Platform D1 to D4, the demo's D2, D3 and D7, section 5.3's table, section 12.2: I can't follow it. The line I keep is that nothing simulated passes as working once YC has the packet. |
| 15 | What we do not know yet | objection | The go/no-go rests on 426 bytes left in your own framework's route budget (27,674 of 28,100), with five kinds still to add on a release two days old and a fallback ticket "the record assumes done". Your framework is gating your demo. |
| 15 | What we do not know yet | question | No date anywhere. When does the recording exist, and who builds the four parts? |
| 15 | Does it fit? | question | Four people refreshing at once would get two errors (429), so "four at once are not a take". For a company platform, that is the sentence a skeptic reads out. |
| 16 | The operator | nit | An encrypted disk, a machine kept off the network and keys that die after one use, all for synthetic data. Thorough, and more than I need. |
| 18 | The packet | objection | A chained audit trail anchored to S3 Object Lock "so an edited packet no longer matches" is built for an auditor. A link I could click would do more for me than a packet I won't open. |
| 18 | The cost | praise | About $170 a month, $17 a rehearsal with $1.10 of it tokens, and "the real month waits on the meter". Cheap, and hedged the right way. |
| 19 | The accepted risks | question | "Accepted because there is one founder." Who else is on this? |

## Questions they would ask

Ranked. Each with the answer the recipient is hoping to hear and what a weak answer would signal.

- **RQ1** When does the recording exist, and what runs today? The status is Proposed and the
  platform's four parts are "designed, not built" (p. 3); chapter 8 orders the work but gives no
  date (p. 15). Hoping for: a date a few weeks out, the spike already run, and the prototype
  canvas opened on a laptop in the meeting. A weak answer signals: months of building before
  anyone sees it, with the design of the demo standing in for the demo.
- **RQ2** Who outside you has used any of this, and which team will be first to use it on its own
  data? The document says "no customer data and no pilot" (p. 13), and the founder runs every
  account (p. 4, p. 19). Hoping for: named teams, each with a champion and a second user, which is
  the brief's own pilot test (definition section 3), and a date for the first. A weak answer
  signals: the company is building toward a YC video rather than toward a user, and the brief's
  own measures (second-person reuse, repeat use without the founder operating it) have no path to
  data.
- **RQ3** If I name a third app now, say a support escalation board from a tickets export, the
  brief's own example, what happens? Hoping for: either it is built from the nine kinds the same
  way, or admission refuses it and names the missing kind, with an offer to show either live. A
  weak answer signals: the platform is the two apps it was built to show, and the registry was cut
  to fit them (p. 9).
- **RQ4** What does the model contribute that a template picker does not? In act 3 it makes one
  call that arranges a definition from nine prebuilt kinds, three views come back from curated
  examples, and both transformations are made by controls, "not by the model" (p. 9, p. 4).
  Hoping for: one crisp sentence on what only the model can do here, such as reading unfamiliar
  columns and turning intent into a definition the platform checks. The closed vocabulary should
  be presented as the safety wedge, with a way to grow it without a release for every request. A
  weak answer signals: "AI-generated apps" is a label on a configuration form.
- **RQ5** In the video itself, how do I tell what is staged, and how long is it? The labels are
  said to live in the rehearsal log and the packet (p. 15, p. 18); act 3 alone is five minutes
  (p. 9); no total runtime is given. Hoping for: an on-screen caption at every seeded, simulated
  or manual step as it happens; a few minutes in total; and generation waits either shown in real
  time or marked as cut. A weak answer signals: an honest demo that will look staged to anyone
  who only watches it.
- **RQ6** Whose data does Jordan's new work read? Page 8 puts both ops-runner-04 events in the
  CSV Maya imported, and page 10 has Jordan start ops-runner-04 work without a source of his own.
  Hoping for: on screen, Jordan selects a source he is separately permitted to read, or imports
  his own. A weak answer signals: reuse is shown on the author's own upload, so the brief's act 5,
  another person's work on compatible data with access judged separately, is not yet shown.
- **RQ7** Why are your own framework and your own sign-in service in the demo's critical path?
  The go/no-go waits on 426 bytes of route budget and a framework ticket assumed done (p. 15).
  fAuth brings a second deployment, a hand-added row and two of the five accepted risks (p. 8,
  p. 19). Hoping for: both can be swapped out for the demo (the budget relaxed, an off-the-shelf
  sign-in behind the same adapter), and the stack pays off in something a viewer sees, such as
  precise edits. A weak answer signals: infrastructure ahead of product. The brief itself names
  that risk when it says another team could build a comparable experience on different
  technology (definition section 15).

## Objections and red flags

What would make the recipient hold or reject. Each with where it comes from.

- **RO1** Nothing in the recording exists yet. The status is Proposed, the platform's four parts
  and the demo's three tools are designed, not built, and there is no date. I am judging a plan
  for a video, not a product (page 1, cover; page 3, summary; page 15, chapter 8).
- **RO2** The case for generality rests on two apps chosen in advance. They run on a registry that
  carries exactly their operators, the network-range lookup and integer currency, while what an
  unscripted request does is "unknown". The brief's own test is an app beyond the supplied demo,
  with no platform code changed (page 4, the partner's first questions; page 9, act 3; page 13,
  chapter 7).
- **RO3** No user and no pilot, and the founder operates every account and every step. The brief
  says customer evidence and a working product decide the strength of the case, and it lists
  repeat use without the founder operating it as a measure; nothing here moves either (page 13,
  chapter 7; page 4, the partner's first questions; page 19, the accepted risks).
- **RO4** The two moments "A Cloud for Small Software" is about are thin on screen: the place a
  company's apps live, and sharing one. Act 1 is a sign-in, not the home. The published app is
  opened by alias because the index lags. No library with both apps closes act 6. And revoking a
  share is the operator's script, not a button (page 8, act 1; page 11, act 5; page 13, act 6).
- **RO5** The proof the summary leads with is off camera, and the scenes on camera are
  engineering ones. The canary and Jordan's 404 on Finance are "in the packet, not on camera", and
  the labels live in the rehearsal log. What the viewer gets instead is a sign-in, a process
  restart, a dropped late answer, a retry and a stale edit (page 3, summary; page 10, act 4;
  page 13, act 6; page 15, security in brief).
- **RO6** The effort goes into proving the recording untampered: image digests, a receipt for
  every number, a hash-chained audit trail anchored to Object Lock, a canary organization, and
  single-use keys on a machine kept off the network. For a solo founder without a first user, I
  would ask where the hours go (page 3, summary; page 16, the operator; page 18, the packet).
- **RO7** The demo's go/no-go depends on the founder's own framework. There are 426 bytes left of
  a 28,100-byte route budget with five kinds to add, on a release two days old, and the fallback
  is a framework ticket the record assumes done (page 15, chapter 8).
- **RO8** The partner's part is written for the engineer. It cites two documents I don't have, uses
  internal labels (S07, D1, A6, TH11, FORMA-232) and packs its sentences with appositives. Its
  diagram labels can't be read at page size and three pages are half empty. It does not fit the
  twenty minutes it claims (pages 3 to 15).

## What they would remember a week later

1. Two different apps built the same way on one build, with no deploy between, and the AI writes a
   definition the platform checks, not code it runs.
2. Unusually honest: every staged step is labeled, the status says "designed, not built", and
   there are no customers yet.
3. A great deal of machinery to prove a demo to an auditor (a canary company, digests, a
   tamper-evident packet), from a solo founder who has not built the platform yet.

## Verdict

hold, medium. The one thing that decides it: nothing on these pages has run. The status is
Proposed, the platform's four parts are designed, not built, and there is no date for the
recording. The plan is the right one, so I would hold for the recording rather than reject. A
third app built from a request the founder did not script would make it an advance.

## Lead's triage

Written by the lead after the recipient's part (2026-10-23). The partner held, medium confidence,
on one point: nothing on these pages has run and the recording has no date. That is the record's
own stated status (Proposed; nothing of the demo prototyped), so it stays; what the read changed is
the demo itself. Five changes went into the record: act 1 opens on the company home and act 6
returns to it with both apps listed; Jordan imports his own evidence (the brief's act 5 and its
completeness clause, "a reused app binds to the new user's permitted data"); Maya revokes his
grant from the share control instead of an operator script; one canary replay is on camera, the
suite in the packet; every staged step is captioned on screen as it happens; and one labeled
unscripted take a rehearsal answers the brief's "an app beyond the supplied demo". The edition and
the notes follow.

| Item | Disposition | Where |
|---|---|---|
| RQ1 | answered | notes.md Q29 |
| RQ2 | answered | notes.md Q30 |
| RQ3 | fixed | 13.2, the labeled unscripted take a rehearsal (platform R4); notes.md Q4 |
| RQ4 | answered | notes.md Q31 |
| RQ5 | fixed | 13.3, every staged step captioned on screen as it happens; notes.md Q7 |
| RQ6 | fixed | 4.1 R6, 5 act 5, 7.1 act 5: Jordan imports his own two-row CSV; notes.md Q28 |
| RQ7 | answered | notes.md Q10, Q32 |
| RO1 | accepted | the record's status is Proposed and nothing of the demo is prototyped (2, 5); the date follows the spike (13.2) and is the head's; notes.md Q29 |
| RO2 | fixed | 13.2, one labeled unscripted take a rehearsal, filed admitted or refused naming the missing kind; the two operators are registered domain capabilities by the contract (C13); notes.md Q4 |
| RO3 | accepted | a pilot on real data is a non-goal (3.2) and the platform's first risk; notes.md Q30 |
| RO4 | fixed | 7.1 act 1 (the company home, its spaces, creating and resuming), 7.2 steps 1 and 7 (home lists the apps with owners and versions), 7.1 act 5 (Maya revokes from the share control, C11) |
| RO5 | fixed | 7.2 step 8 and 13.2 (one canary replay on camera, the suite in the packet); 13.3 (captions) |
| RO6 | accepted | the receipts, the canary and the audit chain are the platform's own mechanisms, not demo extras (D3); the demo adds three scripts (6.7); notes.md Q33 |
| RO7 | accepted | RK1, with the route budget stated as Forma's own share and the kinds' code under the route's JavaScript limit (6.5, 11.2); notes.md Q32 |
| RO8 | fixed | the edition's chapters before the engineer's carry no internal labels and quote nothing the partner lacks; diagram legibility at page size stays accepted (the diagrams are vector) |
