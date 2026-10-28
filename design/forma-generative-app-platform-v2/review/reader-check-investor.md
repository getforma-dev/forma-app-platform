# Reader check: the investor's edition, read cold

Reader: a technical investor at a YC-style firm, engineering background, one hour alone with
`design-investor.md` and the three diagrams it embeds (01, 02, 06), before a partner meeting.
Nothing else was read: not the record, not the briefs, not the input folder.

Edition as read: 8,775 words. At my speed, with the two long lists and three diagrams, that is
fifty to fifty-five minutes with no time to go back. I went back four times; see section 5.

---

## 1. What I could not follow

Listed in reading order. "Unplaced" means I could not connect the sentence to a mechanism, a
definition or a number given earlier in the edition.

### Summary in plain words

- "a publication record and saved work" (goals paragraph). *Publication record* is used as a
  named thing here and in the one-way doors (D2) before anything tells me what it records. I
  inferred it from chapter 2 step 4; the summary should carry a half-sentence.
- "receipts and record commands". *Record command* is first defined in chapter 5. In the
  summary it reads as jargon.
- "60 query operations a second at the busiest moment". The derivation is in chapter 2's fit
  check and holds up, but placed here, before "six linked views" exists, it is a number without
  a unit of work.
- "one program deployed in four roles". I read *role* as an IAM role on first pass, which made
  "deployed with its own credentials" in chapter 2 briefly circular. The sentence in chapter 2,
  "each role boundary is a security boundary, a credential that cannot write what another
  writes", settles it, but too late.

### Chapter 1

- "the gaps the market request names" (the answer sentence). *Market request* is undefined until
  the "Why now" paragraph names the YC Request for Startups. Say "the YC request" in the answer.
- Step 2: "a worksheet lookup that returns its first match. One address has both an exact entry
  and a network-range entry, so one connection can be counted twice". A first-match lookup
  returns one row by construction; it mismatches, it does not double-count. Double-counting comes
  from a join or a SUMIF over overlapping ranges. Either the mechanism is wrong or the sentence
  compresses two different failures (wrong match, double count) into one. I noticed because I
  have written that spreadsheet.
- Step 6: "a 64,000-cent spending table filtered at 20,000 cents down to a 45,000-cent review
  queue". Chapter 5 later says EXP-002 "alone sits above the threshold". So the review queue is
  one expense. Calling one row a queue, and $640 a "spending table", told me the finance fixture
  is a toy before the document admitted it. Not a comprehension failure, but it cost trust early.
- "The product definition calls Forma's alignment with it 'our interpretation'". I do not have a
  product definition. Unplaced citation of a document I was not sent.
- "the sign-in service, fAuth, is a sibling product of the same founder". First mention of fAuth
  in the body is inside a list of constraints. It needs its own sentence: what it is, who runs it,
  and that it is a second product of this founder.

### Chapter 2

- "adopts pages the workspace server renders from typed values". *Adopts* is framework
  vocabulary (hydration, I assume). One word in plain English would do.
- Step 2: "a worker with no network and no keys parses each". Four paragraphs earlier the workers
  are "the only credential with a way out to the model provider". Diagram 02 resolves it (the
  parser is an isolated process inside the worker), but the text contradicts itself for a reader
  who has not yet reached the diagram's small print.
- Step 3: "gets back a typed definition of six linked views". The JSON in chapter 3 shows four
  views, and the text around it says Maya then *adds* the transfer-rate measure, which is already
  in the JSON as `rate`. Four, five or six? Abridged is fine; inconsistent is not.
- Fit check: "Record commits, 0.5 a second on average and tested at 2 a second". Every other
  number in this paragraph is derived in front of me. This one arrives from nowhere.
- Fit check, the gap I could not close: every number assumes "100 ms per warm operation". Chapter
  5 then says a query is "interrupted at a deadline of 1.8 seconds" and the service levels in
  chapter 6 promise a six-view refresh at 2 seconds p99. With at most 2 queries in flight per
  organization, a six-view refresh runs as three waves; three waves of cold queries near the
  deadline is 5.4 seconds, not 2. The edition never reconciles the warm fit check with the cold
  budget, and chapter 7 confirms the cold number is unmeasured. This is the central performance
  claim and I could not make it add up from what is here.
- "Eight job slots, three for imports and five for generation, one of each per organization".
  With ten organizations and five generation slots, half the organizations can be generating at
  once. Fine, but say "at most five organizations generate at once"; "one of each per
  organization" made me think ten.

### Chapter 3

- "N9", "G1", "G6" appear as labels with no index. The preamble promised that only D and TH
  identifiers survive; G and N survive too, and A8 in chapter 6. Either explain the scheme once or
  drop them.
- The JSON shows `"lookup": { "inventory": "cidr" }`. The matching rule that broke the spreadsheet
  in chapter 1, exact entry wins over a range and a connection counts once, is not in the
  definition. Chapter 2 step 2 says Maya confirms it at import. So the rule lives in the import
  mapping, not the app, and a receipt pins a revision, not a mapping. Does the receipt carry the
  mapping? I could not tell, and it is exactly the rule the opening story says must stop living
  in someone's head.
- "The renderer library ships nine kinds" and, two sentences later, "with four kinds measured",
  and in chapter 2, "What is built is the rendering of a four-kind canvas". Does the library of
  nine exist, or four with five planned? I read it three ways and left unsure. This matters
  because the 426 bytes of headroom in chapter 7 is measured over four kinds, and the "proof" app
  Spend Review uses "form, status and detail components that Northstar never used", which may be
  among the five that are not measured.
- "against a budget of 28,100" bytes. Where does the budget come from? The framework, the
  founder, a browser-performance rule of thumb? A budget whose origin I do not know is not a
  constraint I can weigh.
- "a fifth kind that imports certain framework features waits on a framework budget ticket or a
  measurement". Unplaced: which features, which ticket, and why a framework the founder owns
  needs a ticket from itself.
- "the model's five tools hand it identifiers only and the query tool returns a receipt id and a
  row count, never rows", against diagram 01's "explains receipts when policy allows" and chapter
  4's "20 synthetic rows". I could not reconstruct what an *explanation* is built from if the
  model never sees rows. Presumably the receipt's metadata plus synthetic rows; say so.
- "The Northstar demo runs its six acts" against chapter 2's "eight steps". Different units for
  the same demo in adjacent chapters.

### Chapter 4

- "By default the composer sees schemas, catalog metadata and 20 synthetic rows". For the demo
  the rows are synthetic by construction. For a real organization, who makes the synthetic rows,
  and from what? If they are generated from the real data's distribution, that is itself a
  leakage path and needs a sentence. If they are hand-made, generation quality on a real schema
  will be poor. Mechanism missing.
- TH4: "a 9 MiB spreadsheet that inflates past 100 MiB" guarded by "a 2 GiB and 120-second cap".
  The threat number and the cap number are twenty times apart; I assume 100 MiB is the point of
  harm and 2 GiB the ceiling, but the sentence reads as if the cap does not catch the example.
- The accepted-risk paragraph is eleven risks in one paragraph. I could follow each sentence but
  could not hold the set; by "Losing the region" I had lost "Plain HTTP inside the private
  network".

### Chapter 5

- "A receipt pins the definition's digest, the operator versions". *Operator* here means a query
  operator (sum, per_minute). In chapters 4, 6 and 7 *the operator* is the human who runs the
  platform. The collision is unfortunate in the one sentence that defines the product's core
  artefact.
- "the policy epoch" (receipt contents). Chapter 5 later defines a *filter epoch* and a
  *membership epoch* and says "Two counters keep a screen honest". The receipt names a third
  epoch that is never defined. Three epochs, two explained.
- "The record command, decision D5". The preamble says D1 to D4 are the identifiers that survive;
  D5 arrives unannounced, and D14 appears in the chapter 8 map. Not a problem of substance, but
  the preamble's promise is broken.
- Lock order, first step: "the forwarded member's current authorization". Chapter 2 says the
  workspace server is "the one authorization point". Diagram 06 shows the data service checking
  membership FOR SHARE as well. So there are two authorization checks, which is good, but "the one
  authorization point" is then a slogan rather than a fact. Say "the one *decision* point; the
  data service re-verifies inside the transaction".
- Diagram 06, F49: "adoption: compare-and-set on the pinned publication" is drawn from the
  workspace server straight to PostgreSQL under *role workspace*. The text two paragraphs earlier
  says the data service's lock order takes the saved-work row exclusively for "a version
  adoption" and that the data service "alone ... writes records". So who writes an adoption, and
  if the workspace server does, how does the data service's lock order see it? Either the diagram
  or the lock-order paragraph is wrong, or adoption is a saved-work write rather than a record
  write and the text should say that the two walls are different.
- "the canvas polls it every 5 seconds, so a removed member's open display goes dark within 5
  seconds", a figure chapter 6 calls "never traded". A 5-second poll interval gives a worst case
  of 5 seconds plus a round trip; "within 5 seconds" is at best "within about 5 seconds". Small,
  but it is the one number the document says is sacred.

### Chapter 6

- Cost lines: $565 compute, $270 database, about $50 provider. That is $885 of $1,150. The
  remaining $265 is never named, even as "edge, storage, keys, backups and egress". I could place
  it only by subtraction.
- "the availability budget burning at 14.4 times its rate for an hour". I know the SRE burn-rate
  table; a partner without that background will not. One parenthesis.
- "The demo runs as one process on one host" and, in the same paragraph, "The demo's own record
  puts it on the managed database, storage and keys from its first row, so the first three steps
  are true at birth". Both can be true (one process, managed stores) but the paragraph opens by
  listing the database-to-managed migration as a step and closes by saying it is already done.
  Reads as a contradiction until the third read.

### Chapter 7

- "two framework tickets are assumed done but not built". Unnamed. For a risk rated high, I want
  the two tickets in a clause each.
- "a moved framework contract reopens the representation decision". The representation decision
  is D1, which the same chapter calls a one-way door that "cannot be undone without migrating
  every organization". If the founder's own framework can reopen it, it is not one-way in the
  sense the other three are. The two sentences sit four paragraphs apart and contradict.
- "the coding agent shares the pipeline". First and only mention of a coding agent. Which one,
  what access, and is it writing production code? For a risk rated high this is one of the two
  sentences I most wanted expanded.
- "the composer fails the fifteen-request suite in two attempts, so the demo retrieves". I
  understood this as "the demo falls back to retrieval of pre-made apps", which, if so, means the
  demo's generation moment could be staged. Say it plainly.

### Diagrams

- 01: clear. Four people, one system, two outside systems. "Claude" appears in the diagram and
  nowhere in the text, which only says "the model provider". Name it in the text or not at all.
- 02: clear enough at a distance. "Workspace server ... new, on Fieldwork's skeleton". *Fieldwork*
  is never defined anywhere. The caption says "key management is on the deployment diagram",
  which I was not sent; the text in chapter 2 says the stores include "a key-management service",
  so the picture is missing a box the text relies on.
- 06: the boundary labels read "B2 B2 The VPC", "B1 B1 The Internet", "B4 B4 Data service"; the
  prefix is doubled. Cosmetic, but it is the first thing I saw. Substantively, see F49 above.

### Chapters whose question was not answered

- Chapter 1 asks "why build it now" and answers with a framework release date and a blog post.
  Neither is a market reason. The honest answer in the text is "the founder's framework finally
  shipped", which is a founder reason. The question I needed answered, why a team would switch
  from the spreadsheet this quarter, is not attempted; the document says so itself (G7 is
  "fuzzy").
- Chapter 3 asks what shows this is a platform and not one app. The answer is a build-process
  invariant (no deploy between two apps on synthetic data). It shows the platform is *general*;
  it does not show anyone *wants* the second app. The chapter answers the engineering reading of
  its own question and not the investor's.
- Chapter 6 asks "can one person operate it" and answers with how, not whether. Eight containers,
  two zones, three-wave rollouts, six paging conditions, a quarterly restore drill and a
  twenty-four-hour wait on destructive actions, for one person who also maintains the framework
  and the identity service. The chapter never says how many hours a week that is.

## 2. What surprised me or seemed inconsistent

- The honesty. "Every row of data is synthetic, there is no customer, and one founder builds and
  runs it", in the summary, in bold-adjacent position. I have rarely seen a founder lead with it.
  It bought the document a great deal of patience from me, and it is also the whole problem.
- One founder, three products: a web framework (Forma 3.0.0, released two days before this
  document's date), an identity service (fAuth), and this platform. The framework has 426 bytes
  of headroom on its own route budget and the identity service's signing key is on the founder's
  laptop by the document's own account. The platform's two highest-rated risks are the two
  sibling products. Nothing in the edition asks whether building both dependencies in-house was a
  choice worth making, and the alternatives chapter pointer in the map lists "reuse or buy" only
  for the platform.
- The fit check is warm-only and the one unmeasured number is the cold path. The document knows
  this (chapter 7) but chapter 2 still says "carries the design point" without the qualifier.
- Nine kinds versus four kinds versus a budget measured on four. The proof app (Spend Review)
  uses components the measured canvas does not.
- The matching rule from the opening story appears to live in the import mapping, outside the
  typed definition and, as far as I can tell, outside the receipt.
- Diagram 06 has the workspace server writing adoptions directly to PostgreSQL while the text
  gives the data service exclusive locks on the same row.
- "The one authorization point" is checked in two places. Good engineering, wrong slogan.
- D1 is both a one-way door and reopenable by a framework contract change.
- $1,150 a month for ten organizations is cheap and plausible, and $26,000 a month of model spend
  is the stated ceiling if five generation slots run flat out. The per-organization budget that
  stops generation is mentioned but never sized; I do not know whether a customer can spend $50
  or $5,000 of tokens before they are cut off.
- The provider sees "synthetic rows" by default, but for a real customer nobody says who makes
  them.
- The cost of pooled tenancy is stated as "a test suite that must run on every release". The
  undiscussed cost is sales: regulated buyers (and this document's first scenario is security
  logs identifying employees, with a GDPR impact assessment flagged) often will not accept pooled
  tenancy at all, and the "cell" escape hatch is one clause.
- Positioning is one paragraph against one competitor (Retool). No Power Apps with Copilot, no
  Airtable or Notion AI apps, no Hex or Mode, no Glean-style "apps inside an assistant" (which
  the document defers as a non-goal while that is where the incumbents are heading).
- There is no business model anywhere: no price, no buyer, no sales motion. For a design document
  that is correct; for an investor's edition titled as such, it is a hole the reader falls into
  in chapter 1 and never climbs out of.
- The date on the document is tomorrow.

## 3. The five questions I would ask first, in order

1. **Who is the first paying team and what did they say?** You have no customer and no baseline,
   you say so, and you call the seventh goal unmeasurable without a pilot. Name three
   conversations with security or finance leads who saw the spreadsheet story and tell me what
   they said the receipt is worth to them. If there are none, why is the YC demo the first
   audience rather than a design partner?
2. **Show me the cold path.** The fit check assumes 100 ms warm; the deadline is 1.8 s; the
   six-view refresh promise is 2 s p99 under a two-per-organization cap. Have you run the
   million-row aggregate yet? If a cold refresh is 5 seconds, what does the product feel like on
   the first open of every app every morning, and does pre-aggregation at publication change the
   receipt?
3. **Nine kinds or four, and why 28,100 bytes?** Which components exist and are measured today,
   which of them does Spend Review need, and what happens the week a customer wants the tenth
   kind? Who set the budget, and what breaks if you exceed it by 2 KB?
4. **You are three products deep. Which one are you?** Framework, identity service, platform. Both
   siblings are your top risks by your own table. What would it take to buy or adopt identity
   (Auth0, WorkOS, Clerk) and treat the framework as an implementation detail, and what are you
   doing in the next six months that is not maintenance of the two siblings? Who else can ship a
   release, and what is the coding agent allowed to do?
5. **What does the model do on a schema it has never seen?** The fifteen-request suite is on your
   fixtures. Where do the "synthetic rows" come from for a real organization, what is the
   retrieval-versus-generation split you expect in month three, and why would a reviewer trust a
   proposal more than a Retool template or a Copilot-generated Power App?

## 4. Would I take the meeting, and what would decide it

Yes, and I would not pass it to a partner yet.

For: this is the most carefully reasoned pre-seed technical document I have read this year. The
one-way doors are named and defended; the threats are rated before and after controls with
residuals left honestly at medium; the cost is at list with a stated region assumption; every
number I could check added up (the vCPU and memory totals, the per-organization cost, the
burn-rate maths, the restore-versus-budget gap of 21 minutes). The founder can clearly build the
system. The receipt as a product idea, "a number that carries its own provenance and the reviewer
approves data not code", is a real wedge against Retool-class tools if a buyer values it.

Against: no customer, no evidence anyone values it, one person, two self-built dependencies that
are the top risks, and a "why now" that is a framework release. The document optimises for
isolation and generality; it does not say who it is for. The performance claim that matters most
is unmeasured and the edition's own chapters disagree about it. The demo is two toy apps on
synthetic data, and the document hedges that the generation moment may fall back to retrieval.

What decides it, in order: (a) one design-partner conversation I can call; (b) the spike result on
the cold aggregate and the real route size, which the founder says is the first thing to do and
which I would want before the partner meeting rather than after; (c) a straight answer to
question 4. If (a) exists and (b) is within budget, I would bring it to partners as a
founder-quality bet with an unproven market. If neither exists, I would ask the founder to come
back in eight weeks with both, and I would mean it.

## 5. Where the edition is too long or too short for one hour

Too long, in order of how much I would cut:

- Chapter 4, the seven threats in two sentences each plus eleven accepted risks in one paragraph:
  about 900 words. An investor needs TH1, TH2 and TH7 and the sentence "twenty-six more rows are
  in the record". The accepted risks should be a five-item list of the ones that would surprise a
  customer (one operator, pooled roles, plain HTTP inside the VPC, the founder holding fAuth's
  keys, one region).
- Chapter 5's lock-order paragraph and the chapter 2 "roles in a sentence each" paragraph: correct
  and good, but the reader has already been told the guarantee; the mechanism belongs in the
  record. Keep the three-case story (commit, retry, conflict), cut the five-step order.
- The eight-step scenario in chapter 2 and the finance story in chapter 5 overlap on steps 7 and
  8. Tell the finance story once.
- Chapter 6's service-level paragraph: six latency figures, two availability compositions and a
  restore arithmetic. Two sentences would do: the objective, and the fact that the vendors'
  figures clear it only if no full restore happens in a month.
- Chapter 8: the map to the record is for the founder's own process. One line, "section
  references in this edition point into design.md, available on request", replaces both tables.

Too short, in order of what I missed most:

- A paragraph on the buyer and the money: who pays, roughly what, and why this quarter. Even
  "we do not know yet; here is how the demo will find out" is better than silence.
- Competition: one honest paragraph on Power Apps with Copilot, Airtable, Hex and the
  assistant-hosted apps the document defers, not just Retool.
- The founder's load: hours a week on framework, identity and platform, and the plan for a second
  pair of hands. The document rates one-person risk as high three times and never says what it
  would spend money on.
- What exists today, in one screenshot-sized paragraph: "a four-kind canvas renders Maya's
  fixtures under a strict CSP at 27.7 KB; sign-in against fAuth works; nothing else runs." That
  sentence exists but is scattered across chapters 2, 3 and 7.
- The cold-path reconciliation (section 1, chapter 2 above): three sentences that admit the fit
  check is warm and say what the product does when it is not.
- The provenance of synthetic rows for a real customer, and whether the import mapping (the
  matching rule) is inside the receipt.

Length overall: at 8,775 words it fits the hour only if the reader does not stop. Cutting the
items above brings it near 6,000, which leaves time for the diagrams and for the reader to form
the five questions rather than arriving at the meeting with mine.
