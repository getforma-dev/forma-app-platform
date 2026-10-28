# Annotations: forma-generative-app-platform-v2

The per-design layer of the annotated edition (`GUIDE.md` section 8). The method layer, what
each section is for and the course, is `reference/TEACHING.md` and needs no work per task.
This file explains this design to someone learning: why it chose what it chose, the words it
uses, the questions to try before reading on, and where the brief's asks are answered. It adds
no design fact: everything here explains the record, the Forma company app platform on a typed
app definition, and the gate checks that every key below points at a section or an identifier
that exists (M20) and that every link resolves (M21).

Built with `npm run annotate -- forma-generative-app-platform-v2` into
`output/design-annotated.html` (plus `output/flashcards.tsv` for Anki). Keys: a section number
(`6.3`, `10.4`), or an identifier the record defines (`D1`, `TH4`, `SR2`, `N3`).

Reader: a developer with two years' experience who joins to build this platform and has not read a design document before.

## Notes

### 1

**Before you start: the brackets, and the ones you cannot open.** The paragraph above this section, "How to read this record", names the identifier families. Five of them come from documents you will not have: K (the head's rulings), C (the shared contracts between platform and demo), P and A1 to A8 (the validation plan), H (decisions on the earlier design A) and I (the foundation review's gaps). You do not need them: wherever the record relies on one, it restates the content in its own sentence and the bracket is a citation, so "(P06; C04, C06)" at the end of R6 says that R6 is how this record meets product requirement P06 under the query and provenance contracts, and the sentence of R6 itself is the whole rule. Where a contract's content matters, the record carries it: C11's ten permissions are what the trust-level table in 10.2 grants per level, C07's command is 6.3's lock order, C04's grammar is the JSON in 6.2; the record never asks you to open them. The identifiers the record defines (G, A, R, N, SR, D, TH, M, AR, and section 7's P, DS and EE) are links in this edition; hover to read them. The F arrows and the B boundaries are not: keep the figure open for those. "L2" and "L3" on the cover are the factory's document sizes: this record is the full size, the demo's is the smaller one.

**Two words you meet here before the record defines them.** fAuth is a sign-in service the same founder ships separately, live at `auth.getforma.dev` (section 2 says more); "behind an identity adapter" means the platform talks to it through one small piece of code that could be swapped for another provider without the rest noticing. "Pooled" rows, the "control plane", Parquet and DuckDB are all defined by 5.3 and 6; on a first pass take them as "one shared database with the organization id on every row", "the role that publishes", "an immutable file format for evidence" and "the engine that reads it".

**What is unusual about this summary.** It sells nothing: the "at the expense of" clause gives up expressiveness (a widget the vocabulary lacks is an engineering release) and generation latency (every proposal is a job you wait for), and the rest of the record keeps both refusals. Compare the predecessor it revises, a security-first platform whose model wrote markup; this one makes the model propose data, and the summary says that in its first sentence because everything else follows from it.

**Pitfall.** Reading "one binary in four roles" as one service. It is one program whose credential decides which role it runs, deployed four times with disjoint database grants (D11, 6). The separation is in the credentials, not the code.

### 2

**Two things called Forma.** Forma 3.0.0 is a web framework (a compiler, a server renderer, a browser runtime) that deliberately ships no identity, storage, jobs or tenancy. Forma the platform is what this record designs on top of it. When a sentence says Forma "withholds" something, it is the framework; when it says Forma "publishes" something, it is the platform. The prototypes in the second paragraph are three experiments on the older 2.x framework; their widgets mount through a 2.x call the 3.0.0 runtime no longer has, which is one reason they are donors of code and not a base to extend. "A 0 FAIL gate" is the framework's own release check with nothing failing; "loopback" means the prototype answers only its own machine; a "fixture" is fixed synthetic data with known expected numbers that tests run against.

**Why the problem is six numbered steps and not an adjective.** Each step names a mechanism that a later decision answers: the exported rows with no revision (step 1) become immutable revisions with confirmed mappings (R3, D4); the match rule in someone's head (step 2) becomes a confirmed interpretation and a registered lookup operator (6.3); the missing denominator (step 3) becomes a receipt on every number (R6, G3); the rows pasted into a chat (step 4) become a provider that sees schemas and synthetic rows only (N8, D8); the screenshot in an email (step 5) becomes saved work with its own share grant (D14, R11); the overwritable cell (step 6) becomes a record command with a version (D5). When you write a context section, number the failures so the decisions can point back at them.

**Read the constraints as the walls of the room.** The compiler accepts only a subset of TypeScript for components that render on the server; the record gives you the rule you will meet first (a computed comparison such as `kind === "summary"` is refused, BUILD-610, so the server sends booleans and the renderer branches on them) and names the other diagnostics in appendix B, and the compiler's own messages are the reference for the rest. A route's framework code may not exceed 28,100 B gzip and the spike's canvas already used 27,674 B. Those two facts shape the whole browser design (D10, 6.5): there is 426 B of headroom, which is why "a new primitive is a release" is a budget question, not only a policy.

### 3.1

**Notice the failure sentence.** "A security-only product disguised as a platform" is the first failure named because it is what the predecessor, design A, risked becoming; "hidden per-app coding" is the second because it is how most platforms quietly fail their generality claim. A goals section that names how it could fail gives the reviewer something to argue with. G7's "fuzzy" is the same honesty the other way: nobody can measure "cheaper than the spreadsheet" without a pilot.

**Words G1 to G4 use before the record defines them.** Row-level security is the database refusing rows that are not the session's organization (8); compare-and-set is "change it only if it still equals what I last saw" (D14); an idempotency key is the client-chosen id that makes a retried command the same command (9); a half-open window includes its start and excludes its end. All four are on the Glossary tab.

### 3.2

**Why a non-goal carries a reason.** Each item is something a reader might ask for, and the reason is what it would cost now: a public surface adds threats before it adds proof; live coediting breaks the authority model because an unsubmitted mutation has no submitter; a marketplace needs content rights nobody has settled. The markup non-goal is the big one: dropping code and markup generation means no sanitizer, no untrusted compiled module to admit, no translator, and two of the worst threat rows gone. That saving is the argument for D1.

### 3.3

**Why an envelope and not a forecast.** With no customer there is nothing to forecast, so the record borrows the validation plan's envelope (A1 to A8) and adds its own (A9 to A18), each with an "if wrong" clause that says which section moves. That clause is the useful part: a reviewer who disagrees with A2 can see that queue controls change and authorization does not.

**Notice where 3.3 comes from.** A15's peak factor is 100 seats over 30 concurrent users, not a number pulled from the air. A3's 43.95 GiB is three months of originals plus three months of a normalized copy budgeted at twice their size (section 8 says Parquet is budgeted at twice until measured). A13's "two product definitions" are the controlling one on the cover and design A's earlier, security-first one (appendix B, "Earlier records"); where they differ, the cover's wins.

### 4

**Why a stakeholder table opens the requirements.** Each row says which sections answer one reader's concern, so a reviewer, an administrator or the operator can read only their part. Notice the last row: the head and the demo's lead care about "two apps, one path, no platform code between". That is N9 and R14 later, and it is the claim the whole release is judged on.

### 4.1

**How to read an R line.** MUST, SHOULD and MAY are the words of [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119): a MUST that fails is a defect, a MAY is permission. Every line ends with the product requirement (P) and the contracts (C) it answers. "Programmatic parties are callees" in the first paragraph means that in this release programs are only ever called by the platform (fAuth, the provider, the cloud); none calls in, so there is no API client, no issued token and no machine user to authorize (10.5 says the same from the security side).

**What is unusual in R14 and R15.** Most requirement lists say what the system does; R14 says what the code may not contain, a branch on an organization, space or app name, because the generality claim can only be proved as an absence. R15 puts token verification on the platform rather than trusting the sign-in service: fAuth authenticates the person once, and the platform checks issuer, audience, signature, expiry, revocation and membership on every request after.

**Pitfall.** Reading R6's "a failed calculation is an error, never a zero" as pedantry. A zero in a total is the spreadsheet failure from section 2 wearing a server's clothes.

### 4.2

**Follow the arithmetic once.** Thirty concurrent users, one interaction every 10 s, is 3 interactions a second; six linked views make that 18 query operations a second; at peak all 100 seats are active, 3.3 times more, so 10 interactions and 60 operations a second. Commits: one a minute a user is 0.5 a second. Every later capacity number in 5.2, 6 and 11.2 grows from these four.

**Why percentiles and not averages.** "p99 under 2 s" says one request in a hundred may be slower and admits it. The six-view refresh row adds a subtlety: six operations must all land for the refresh to count, so each is held to p99.83, because 0.9983 to the sixth power is about 0.99. A target that is a composite of several calls must be tightened per call or it is not the target it claims to be. [Service Level Objectives (SRE book)](https://sre.google/sre-book/service-level-objectives/) explains why percentiles.

**The scenario form.** "Source, stimulus, environment, artifact, response, measure" is the six-part quality scenario; the four scenarios compress it into prose. Unpack the first: source, a practitioner; stimulus, selecting a destination in one of six views; environment, peak load on a 1-million-record source; artifact, the six linked views; response, all six answer from one filter epoch with no older result shown; measure, p99 under 2 s at the client under open-loop load at twice peak. Reading a scenario this way tells you what the test harness must do.

**The SR list, and its order.** SR1 to SR13 are the security requirements section 10 emits; each cites M-numbers, the mitigations of the threat table, which only mean something after 10.4 and appendix D. Two things help now: each SR's test line is readable on its own (forged token refused, canary gets nothing, an unsigned image refused), and SR13 sits before SR12 only because the list ends on the fail-closed rule the record wants last; the numbers are stable identifiers, not an order of importance.

### N3

**Why the latency table is measured at the client.** Authorization and the network are inside the number because that is what the member feels, and the record names the sample size, 10,800 interactions a business hour, so a p99 is statistically meaningful (one percent of 10,800 is 108 slow requests). First paint uses a different scale, the 75th percentile of Core Web Vitals, because that is how [LCP](https://web.dev/articles/lcp) is assessed.

### N5

**What "linearizable per record" buys and what it does not.** Each record behaves as if its commands ran one at a time in a single order, enforced by one PostgreSQL transaction per command; nothing is promised across records, and search may lag a minute. The "membership epoch" here is the organization's revocation counter, defined in 6.1 and appendix A: a command is checked against it inside the same transaction. Narrow guarantees are cheaper to keep and easier to test: G4's retry and conflict tests are the proof.

### SR2

**Why "offline" matters.** The adapter verifies fAuth's signed token against a cached key set, so a request never waits on fAuth and fAuth being down stops new sign-ins only (6.7). The epoch re-check per request is what keeps that cache from becoming a revocation hole. PKCE ([RFC 7636](https://datatracker.ietf.org/doc/html/rfc7636)) is the proof-of-possession step that stops a stolen authorization code from being exchanged by someone else; it is also why the platform's client at fAuth has no secret.

### SR3

**Why a signed token between roles.** Inside the private network nothing is trusted on sight (the assumption 10.1 calls SA2). Every call carries a short-lived token naming who calls whom for which principal, and the callee re-reads membership. The test is the telling part: the `workspace` role must be unable to insert a publication at the database, so the separation holds even if the token check had a bug.

### SR5

**What "no system interface" means.** The parser is compiled to WebAssembly and run under a runtime that offers it only an input stream and an output stream: no file system, no sockets, no clock to speak of, and so no path to the task's credentials endpoint. A WebAssembly module can only do what its host hands it; [WASI](https://wasi.dev/) is the standard set of such interfaces, and this design hands over almost none of it.

### SR7

**Why re-authorize before release.** A result may have been computed before a revocation, or found in a cache. Checking membership again at the moment of release, not only at the moment of asking, is what makes the 5 s revocation bound true for queued work. The idempotency key's scope, organization, principal, work and command, is what stops one principal reading another's outcome (the threat row for that is in appendix D).

### 5

**Read the scenario as the test the design must pass.** Eight steps, each tagged with the decisions and requirements it exercises and whether that part is [built] or [proposed]. Steps 5 to 7 carry the fixture numbers (1,140,000 bytes, 114,000 a minute, 64,000 cents, EXP-002 at version 1) that section 7 and the drills in 11.4 reuse; the record is consistent with itself because one story runs through it. "Typed props" in step 3 are the plain values (flags, lists, numbers) the server computes for a renderer; appendix A defines them.

**The "like X, unlike X" sentence.** Retool is the nearest thing a reader knows. The difference is placed exactly: there an app is code the platform trusts; here it is data the platform validates and renders. Everything in sections 6 to 10 follows from that one difference.

### 5.1

**What is unusual in this context picture.** Most platforms' context diagrams show an identity provider and a model provider as two more integrations. Here they are drawn as seeing opposite halves of the world on purpose, identity without data and data without identity, and the record then holds every later component to that split (N8, the workers' egress in 6.4). The simplifications listed under the picture are worth copying: a diagram that says what it left out can be trusted about what it kept.

### 5.2

**The word "adopts".** "The browser workspace adopts pages the workspace server renders" is the framework's word for the browser attaching its island code to HTML the server already produced, what other frameworks call hydration. It has nothing to do with "adoption" of saved work (D14, appendix A), which is moving a case to a newer publication. The two collide in this record; keep them apart.

**How the fit check works.** Little's law says the number of requests in flight equals the arrival rate times the time each takes: 60 operations a second times 0.1 s is 6 in flight. One data node allows 8 at once, so the design point fits one node with room, and the second node exists for availability (N2), not load. The per-organization cap of 2 means a six-view refresh runs as three waves of two, about 300 ms warm. All of that rests on "100 ms a warm operation, assumed until measured"; the spike is where it becomes a fact. [Little's law](https://en.wikipedia.org/wiki/Little%27s_law) is one line of algebra worth memorising.

**How the availability number is built, and which factor is ours.** The published-app path is a chain of hard dependencies: edge, workspace server, data service, PostgreSQL, plus object storage and key management when a query is cold. The vendors publish their figures; the two 99.9% factors for the platform's own roles are the record's assumption, marked as such, resting on two tasks in two zones each sized to carry the load alone (a shape called [static stability](https://builder.aws.com/content/3K9SiqiJ369TGkkGzqPM95BuYWI/static-stability-using-availability-zones), which 11.1 names). Multiply and the warm chain is about 99.65%, the cold about 99.55%, both above N2's 99.5%. Then the subtraction most documents skip: 99.5% of a month is 3.65 hours, and one full restore at the 4-hour RTO would overspend it by 21 minutes, so the objective assumes no full restore in a month. RDS Multi-AZ, CloudFront, S3 and KMS are the managed database, content network, object store and key service D15 chooses; 6.6 describes each.

**Pitfall.** Assuming the second data task doubles capacity. Organizations hash to a preferred task so a revision is decrypted once; the second task is the spare, and N1's 10x adds tasks rather than relying on it.

### 5.3

**Why these four are one-way.** The test the record applies is: what is an organization's stored data made of? Every stored app is in the typed definition (D1); the organization's evidence of what runs is the publication record (D2); every key in every store carries the organization id (D3); receipts and saved work are made of Parquet revisions and PostgreSQL rows (D4). Changing any of them means migrating every organization at once, which is what expensive-to-reverse means. The record's "one-way door" is Amazon's phrase from the [2015 shareholder letter](https://s2.q4cdn.com/299287126/files/doc_financials/annual/2015-Letter-to-Shareholders.PDF).

**Notice the half-door.** D15, the cloud and region, is two-way for the platform and one-way per organization once its data lives in the region, because moving it is that organization's migration; 10.7's accepted risks add that its backups stay in the region too. A door can be two-way for you and one-way for your customer.

**Read the "Binds demo" column.** The Northstar demo must make the same choice wherever this column says yes, so that it can grow into the platform without a rewrite (C16). This is how a small version and a large one stay the same system: shared doors, not shared code.

### D5

**Why "two-way at the protocol".** The record command's order of checks can change release by release because it is enforced inside one transaction the data service owns; callers see only an outcome, a conflict or a retry result. Compare D1, where every stored app is in the format: a protocol can be versioned, a stored shape must be migrated.

### D6

**Why the organization comes from `tenant_id` and nothing else.** The session is the only source of the organization id for the whole request; 10.2 later states this as the first invariant. If a header, a path or a body could name it, every handler would have to be checked for honouring the wrong one. One source, verified once, makes the later walls (row-level security, prefixes, key policies) checkable.

### D8

**Why generation is a job.** Three reasons share the row: a provider outage must not hold a request open (R13), whole-output validation needs the whole output before anything is stored, and a job has visible state the member can see within a second. The cost is the generation latency the summary admits.

### D10

**Why boolean kind flags and server-built SVG.** The compiler refused a computed kind comparison (BUILD-610), so the server declares booleans and the renderer dispatches on them; a timeline built from numbers rather than strings means no string is ever interpreted as markup, which is SR13's rule and, in appendix D, the last threat row. The "426 B of route headroom" cost is literal: the next library import may not fit.

### D14

**Why adoption is explicit.** Saved work pins the publication it was started on. If work moved to a new publication automatically, a changed calculation would silently rewrite earlier findings, which the controlling definition forbids (section 12 of the definition, cited in alternative S). Compare-and-set makes adoption one visible act that either happens once or fails with a 412.

### D15

**Why a cost column says "money".** Managed services (RDS, S3, KMS, Fargate) buy one operator a lot of operations, and the price is the cost table in 11.2. The alternative, Kubernetes or provider-neutral hosting, optimises for portability nobody has asked for yet (A9).

### 6

**What is unusual in these cards.** Each has an "axis of change" line, the one kind of change that should land in this box and nowhere else: member-facing workflow in the workspace server, publication policy in the control plane, the grammar and the command in the data service, the generation pipeline in the workers, the component library in the browser. When you are asked to add something, that line tells you which card you are editing; if it is two, the design has a seam you should raise. The second unusual thing: the three boundaries between the cards are database roles (`workspace`, `control`, `data`), so the view server cannot write a publication and the publisher cannot read a record because PostgreSQL refuses, not because the code remembers to.

### 6.1

**Why one authorization point.** A check made in one place is tested in one place, and the canary suite replays every route against it (G5). The trick worth copying is in "what can go wrong": no store call accepts anything but the principal type the authorizer returns, so a handler that forgets to authorize cannot compile its way to the database.

**Why membership is never cached.** Sixty indexed reads a second at peak is what a cache would save; the record spends them so that revocation within 5 s is proved by a read rather than bounded by a cache lifetime. When a safety property depends on freshness, pay for the read.

**"The role's hourly KMS-certified key".** Each role keeps a signing key pair in memory and replaces it every hour; the role's long-lived asymmetric key in the key service signs a short statement vouching for the new public key, which is what "certifies" means. Callees trust the key-service key, so they accept tokens signed by any hourly key it has vouched for, and a stolen in-memory key is useless after the hour. The design signs with a key no process can export and still never calls the key service on the request path.

**Notice a denial is 404, and the cookie.** The same answer for "not yours" and "does not exist" means the API never confirms that an identifier is real (section 9). The platform session is an opaque `__Host-` cookie; the prefix tells the browser to accept it only over HTTPS, from the host that set it, with no domain attribute ([Set-Cookie prefixes](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie)). fAuth's token is used once at the callback and never presented again. The `oauth_clients` row is fAuth's registration of the platform as a public PKCE client: a row, no secret.

### 6.2

**Why the publisher must not touch model output.** The role that parses what the provider returns is the role most exposed to hostile input (section 10's actors include the model itself and text planted in evidence). Keeping it one database grant away from inserting a publication means a flaw there cannot become a publication nobody approved. That is the whole reason this is a separate box.

**Why SERIALIZABLE here and READ COMMITTED in 6.3.** The approval's premise spans three rows (the draft's digest, the policy version, the approver's epoch). Under read committed, two transactions can each read a consistent-looking premise and both commit, which is called write skew; SERIALIZABLE makes PostgreSQL detect it and abort one. The record store (6.3) locks every premise row explicitly instead, so it can stay at read committed. Both are valid; the choice follows whether the premise rows can all be locked. [Transaction isolation (PostgreSQL)](https://www.postgresql.org/docs/current/transaction-iso.html) defines the levels.

**The adoption table, pictured.** At publication the publisher writes one row per earlier publication of the same app whose record schema the new one can still read: (app, new publication, earlier publication). Later, when a practitioner's pinned work asks to adopt, the workspace server looks up (app, new, pinned) and finds a row or does not; no schema comparison runs at adoption time, and an incompatible schema is blocked without a migration (R10). A compatibility decision made once, at publication, by the role that owns publications.

**Notice the CHECK constraint, and read the JSON once.** Approver not equal to author is enforced in the code and again as a database CHECK, so the rule holds even if the code does not. The JSON names kinds, measures, a filter and a record schema with transitions; nothing in it is code, a URL, a path or a grant, because the grammar has no such type. That is what "the model proposes data" means concretely.

### 6.3

**What a receipt is, and why it is also the cache key.** A receipt pins everything a result depended on: the definition digest, operator versions, revision digests, the record snapshot's commit sequence, the parameters, the policy epoch, and the result's own digest. Hash all of that and you have a key that can never be stale, because any change to any input changes the key. So the result cache buys latency only, and authorization is still re-checked before a hit is released.

**The lock order, and why it is the order it is.** Authorization first; then the idempotency outcome, inserted under a unique constraint, so a duplicate serialises on that row and gets the stored answer; then the membership and saved-work rows `FOR SHARE`, so a revocation or adoption taking them `FOR UPDATE` must wait or has already won; then the record row `FOR UPDATE` and the version comparison; then the writes. Outcome before version is the subtle part: a retry after a lost response carries a now-stale expected version and must get its original outcome, not a conflict. [Explicit locking (PostgreSQL)](https://www.postgresql.org/docs/current/explicit-locking.html) explains `FOR SHARE` and `FOR UPDATE` and the advisory locks the audit chains use.

**How DuckDB is boxed in, and what a per-query S3 session is.** Each operation is compiled from the admitted syntax tree, never from SQL text, into a read-only instance with extension autoloading off, no `httpfs`, external access limited to the organization's cache directory, two threads, 1 GiB and a deadline that CI tests actually interrupts. Every item on that list closes one way an engine flaw could reach another organization's files. The cold fetch adds a wall outside the engine: the data service asks the cloud for temporary credentials valid for one query and only for that organization's prefix, so even a wrong path is refused by the store, not merely by our code. [Securing DuckDB](https://duckdb.org/docs/stable/operations_manual/securing_duckdb/overview) is the vendor's own list of the same knobs.

**Money.** Integer minor units with a currency code and exponent in the grammar, and a sum across codes refused rather than converted (N6). Floating-point money is the spreadsheet failure again.

### 6.4

**Why a table is the queue.** A claim, its result and its audit row commit in one transaction, which a separate queue service cannot give you. `FOR UPDATE SKIP LOCKED` lets many workers take different rows without blocking; the 120 s lease with a claim sequence that fences the result write is what stops a paused worker's late write from landing after another worker took the job.

**The sandbox and the composer are in this box for the same reason.** The worker is the only credential with egress to the provider and the only place an untrusted file is opened, so both dangers live behind one credential that holds no member session and reads no revision rows. Notice what the model's `query` tool returns: a receipt id and a row count, never rows, so no evidence re-enters the prompt (N8).

**Notice "provider request id before the call".** A worker that dies after calling the provider would otherwise cause a second call and a second bill; recording the id first lets the next claimant wait for the result. The remaining double payment is accepted in writing (10.7's list).

### 6.5

**Why the browser computes nothing.** It holds only the session cookie, "adopts" what the server rendered (attaches its code to the server's HTML; see the 5.2 note) and makes it a workspace. The filter epoch is the one piece of logic worth understanding: a filter change increments a counter, every query carries it, and any response with an older counter is dropped, so six linked views never show numbers from two different filters.

**Forms before activation.** A server-rendered form carries an `Idempotency-Key` the view server minted at render, so a submit before the island's JavaScript loads is the same command, once. Progressive enhancement is here a correctness property, not only an accessibility one (N12).

**The ceiling is a byte count.** 27,674 of 28,100 B with four primitives; the fifth may not fit. `createResource` is a framework data-loading primitive and the HTML runtime is the framework's module for mounting markup: either import pulls more framework code into the route, which is why the record names them as the two things a fifth primitive must not need. Section 11.1 says a release keeps every renderer version a current publication binds, and FORMA-232's budget gates every library addition.

### 6.6

**Why the edge authenticates nobody.** It stops floods and serves assets near the member; identity is decided once, behind it, at the workspace server. Two places deciding identity would be two places to get wrong.

**Read "acknowledged at the synchronous standby".** A commit returns only after a second zone has the bytes, so a committed record survives a zone, at the price of a longer commit (the assumed 20 ms that caps a hot record near 50 commits a second). [Multi-AZ deployments (RDS)](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Concepts.MultiAZ.html) is the vendor's description.

**One key per organization.** Deleting the key makes every object under it unreadable at once, immutable Parquet included; that is how deletion works in a store that never updates (section 8). [KMS key deletion](https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html) describes the waiting period the record cites.

### 6.7

**What is unusual here.** Two rows are "hard if changed" rather than hard or soft: Forma 3.0.0 and DuckDB are linked into the binary, so they cannot be down, but an upgrade of either is treated as a dependency event, with the consequence named (a moved framework contract reopens the typed-definition ruling; a DuckDB upgrade changes query plans but not receipts). Most dependency tables forget the libraries. The other unusual row is the last: the shared formats are a dependency of every role and the browser, versioned like the API, so a callee-first rollout can meet the previous release's form for one release.

**Notice "a timeout is a refusal, never a retry inside a request".** Retrying inside a request multiplies load exactly when a dependency is struggling; the browser retries only a 503 with `Retry-After` (section 9).

### 7

**How to read a flow without the picture in your head.** Each flow is numbered phases, and each phase cites the arrows it uses (F2, F3, F15). The arrows live on the diagram above the flow, and the sentence under each diagram, "What matters", names the one or two arrows the section's argument depends on; read that sentence, find those arrows on the picture, and then read the phases. In this edition the P, DS, EE and B elements show their labels on hover; the F numbers do not, so keep the figure open for them. B7, the organization boundary, is the one boundary never drawn, because it is inside every store: every table, prefix and key is partitioned by organization, so every arrow crosses it.

### 7.1

**What flow 1 proves.** A revision, a draft and a publication are each written by exactly one role, and F12 is the only arrow leaving the private network. If you remember one arrow in the whole record, remember F12: schemas, catalog metadata, synthetic rows and the intent cross it, never evidence.

**Read the failure cases as the threat model's raw material.** A bomb file, a provider timeout, a stale patch, an author publishing her own work: each failure here reappears as a threat row in 10.4 or a requirement. Flows are where the threat model comes from, so what you predict here you will recognise there.

### 7.2

**Why every result crosses F34 twice.** Once to read the record snapshot the receipt pins, once to re-check membership before the result is released. The second crossing is what makes "revocation during a queued query" end in a denial and not a leak.

**Notice the sign-in is drawn once.** Every flow shares it, so it is told at the top of flow 2 and assumed elsewhere; a reader who wants the checks goes to 10.5.

**The copied link.** Without a grant the workspace server resolves the id inside the session's organization, finds nothing and answers 404, the same as for an id that never existed (R11). Links carry identifiers, not access.

### 7.3

**Step 2, unpacked.** The workspace server reads the session and the epoch, checks that this principal may write records on this work, then compares the submitted fields and the requested status transition with the record schema of the publication the work is pinned to, which it reads from PostgreSQL; anything the schema does not declare is rejected before the command is forwarded. The data service will validate again inside the transaction; the workspace server's check is the early, cheap refusal.

**Read the sequence diagram line by line.** Messages 1 to 8 are the happy path; 9 to 14 the retry after a lost response, where the outcome for `ik-1` is found before any version check so the stale expected version is never applied; 15 to 19 a new key `ik-3` with a stale version, where the version check runs and fails into a 409 with an authorized recovery view. One diagram carries the three behaviours G4 promises.

**Why the canary never reaches P4.** The workspace server scopes every lookup to the session's organization and finds nothing, so the data service is not even called. Isolation holds at the first wall; the second wall, row-level security, is for the day the first has a bug.

### 8

**The two halves.** Small mutable references with cross-row invariants, enforced by PostgreSQL; immutable bulk bytes named by digest, in object storage. Each store's row in the table says whether it is of record or derived: derived data (index files, caches, previews) can be rebuilt, so backups and the strictest acknowledgement go to the record data only. "Read-after-write, monotonic" in the first row means a reader sees its own commit and never sees time go backwards; both hold only on the primary, which is why sessions and epochs never move to a replica.

**Why the organization id leads every key and every index.** It is the partition key: queries are cheap when they filter on it, no foreign key can cross it, and row-level security has one column to check. `FORCE ROW LEVEL SECURITY` closes the exemption PostgreSQL grants table owners, and the `NOLOGIN` owner means no connection holds it anyway; the other exemption, superusers and `BYPASSRLS` roles, no table setting closes, which is why no runtime role carries it; the organization id reaches the policy through a per-transaction setting that the pool resets, and a missing setting denies. [Row security policies (PostgreSQL)](https://www.postgresql.org/docs/current/ddl-rowsecurity.html) is the page behind each of those words. "Every index is B-tree, local" says there is one unpartitioned table per entity with ordinary indexes on it, as opposed to a partitioned table with an index per partition; "a cap, not a split" says a record hotter than about 50 commits a second is throttled (429 past 20 waiting) rather than split into several rows.

**Parquet row groups.** Files are sorted on the confirmed time column in groups of 100,000 rows, so a ten-minute window over a million rows reads only the groups that overlap it. That is how a columnar file makes a range query cheap. [Parquet file format](https://parquet.apache.org/docs/file-format/) shows the structure.

**Retention is where deletion is designed.** Crypto-shredding (disable the key, then delete it after the service minimum), a 24-hour hold any administrator can cancel, tombstones that carry no payload, and the deletion ledger replayed after a restore before the environment serves. A design that stores immutable files must say how it forgets.

### 9

**What is unusual in the error model.** Two deliberate asymmetries. Denied, foreign and absent are all 404, so that existence never leaks, but an author approving her own draft gets 403 naming the rule, because she already sees the app and a named rule tells her what to do. And a reused key with a different payload is 409, where the [IETF idempotency-key draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/) suggests 422, because the record sees the conflict as being with the earlier request, not with this request's shape. Errors are [problem details](https://datatracker.ietf.org/doc/html/rfc9457) with a stable code per class, so clients branch on the code and never on the text.

**"Tags never reused".** In the API's and the shared formats' encoding every field has an identifier (a name or number) on the wire; when a field is removed its tag is retired for good, so an old client that still knows the tag can never misread a new field as the old one. With "fields added only with defaults" this is what lets an old tab meet a new API after a release.

**Notice the organization never comes from the request.** Not a body, not a header, not a path. Section 9 is where that rule is stated for callers; 10.2's first invariant is where it is enforced.

### 10

**What is unusual in this threat model.** It is built from section 7's arrows rather than from a checklist: every row names the elements and boundaries it crosses, and the worked record of 6 and 7 is cited by element, so a row that does not trace is a finding. The rating scale is defined in one paragraph before the table (what High means on each axis) so that 10.4, appendix D and 13.1 are comparable, and the likelihood is rated against what is already built, with a deferred mitigation lowering nothing. Read 10.2 before 10.4 or the rows will not parse.

### 10.1

**Why SA1 matters more than it looks.** Every role runs the one binary, so a flaw in one is in all; the defence is per-role credentials and checks at every callee, never "the other service would have caught it". SA2 says the same from the caller's side: no internal caller is trusted.

**The data protection sentence.** Everything is synthetic today, so no regime applies; the record still says what happens before the first real row: the organization, as controller, needs an impact assessment, because connection logs identify employees. Writing the obligation down now is cheaper than discovering it with a customer.

### 10.2

**Trust levels include the machines.** TL6 is an untrusted author (a dropped file, the model's proposal until admitted) and TL7 the four service identities. A threat model that lists only humans misses that a compromised role holds TL7's authority.

**The entry-point table's authentication column.** Four different mechanisms, one per kind of caller: a session cookie plus a same-origin header for people; a PKCE code with `state` for the sign-in callback; the signed service token for role-to-role calls; and, for the cloud's own stores, SigV4, the cloud's request signing with the task role's credentials, carrying "the organization's context", the encryption-context name the key policy requires, so a call for the wrong organization fails at the key. "The SDK" is the vendor's client library that does that signing. Each row also says where the input is validated, which is the column to check when you add an endpoint.

**The six invariants are the trusted computing base.** Each names what must hold and which components enforce it; those components, and only those, are trusted. Notice what is excluded: the composer, the model, the index and the island. The island drops late pages, but the server re-checks at execution and delivery, so the island's discard is a convenience, not a guarantee. [Building Secure and Reliable Systems](https://google.github.io/building-secure-and-reliable-systems/raw/toc.html), chapter 6, is the free source for the idea.

**Blast radius per boundary.** The paragraph after the assets table says what each compromised boundary reaches and, as important, what it does not: a compromised workspace server reads what any member may read but holds no publication, record, Parquet or provider key. Write this paragraph for your own designs; it is the test of whether the role split does anything.

### 10.3

**Why TA10 is on the list.** Future management, an acquirer or a court is an actor with legal power over retained data. It cannot be mitigated with a control, only with minimisation and short retention (TH27, M48), which is why the record keeps 90 days and tombstones with no payload. Most threat models stop at TA9; this one names the actor that arrives with a subpoena or a term sheet.

### 10.4

**Read a row as a sentence.** Actor with prerequisite can do action, which leads to impact, negatively impacting asset. The invariant column says what is at stake, the existing control what is already built, the response what this record adds, split into a prevent control and a detect control, and the last column rates likelihood and impact before and after. High is defined on both scales in the paragraph above the table so the ratings are comparable; "a deferred one lowering nothing" there means a mitigation marked deferred does not reduce the rating. [STRIDE threats (Microsoft)](https://learn.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats) defines the six letters.

**Why only seven rows here.** These are the High-impact threats whose controls this record designs; the other twenty-two are in appendix D on the same scale. Padding a threat table with TLS rows hides the ones that matter.

**The attack tree.** AG1 is broken into five branches, each naming its rows. Branch 1 meets two independent walls; branch 2 is bounded to one member; branch 3 is where one flaw reaches every organization, which is why the canary suite runs every release. A tree makes it visible which branches share a control and which do not.

### TH1

**What is unusual in this row.** The existing-control column names something the platform deliberately ignores: fAuth accepts a "slug" header, a request header naming an organization by its short name, as a fallback for finding the tenant, and the platform never reads it, taking the organization from the signed token only. A control you refuse to use is still a design decision worth writing down. The detect half is also specific: an alert at 10 cross-organization identifiers an hour, because guessing identifiers is noisy and the canary suite only runs at release.

### TH3

**The refresh-cookie bridge.** fAuth keeps its own refresh cookie on its origin, set `SameSite=None` so a browser would send it from other sites, which a sign-in service needs. The platform could have leaned on it to keep members signed in across its own session's life; it does not, because a cross-site cookie is exactly what a lure link exploits. Sessions are the platform's own, and the one-time code plus PKCE and `state` is all that crosses from fAuth.

### TH4

**Why the existing control column says "none".** The prototypes' 5 MiB browser cap is not this system's; a design must not borrow controls from code it is replacing. The mitigation is structural rather than a size check alone: the parser cannot reach the network, the credentials endpoint or any file, so even a successful exploit holds one job's bytes.

### TH5

**What an approval is.** Not "approve draft 7" but the reviewer's own session over the admitted content digest, checked inside the publisher's SERIALIZABLE transaction against the draft's current digest, the policy version and the approver's epoch. Any later patch changes the digest and voids the approval; a replay after the reviewer's revocation fails the epoch check. The inbox renders the admitted bytes, so what the reviewer saw is what is published.

### TH6

**The supply chain row.** One binary every role runs means one hostile dependency reaches every organization at once, which is why the rating stays Medium after mitigation (AR8). Notice the sentence about the coding agent: its identity cannot tag, edit workflows or merge. Who may create a release tag is a security control. [Sigstore](https://docs.sigstore.dev/) is the keyless signing the row names; [CycloneDX](https://cyclonedx.org/) the SBOM format.

### TH10

**Why a cache is a threat row.** A cache keyed without the organization or the policy epoch serves one scope's answer to another, or a pre-revocation answer after it. The receipt digest as the key carries both, and the hit is still re-authorized before release. Caching is an authorization decision, not a performance one.

### TH21

**The revocation race, as locks.** A command holds the membership row `FOR SHARE`; a revocation takes it `FOR UPDATE`. The two cannot overlap, so either the command commits under the old epoch before the revocation (and stays in authorized history, with visibility re-checked at delivery) or the revocation lands first and the command sees a stale epoch and is refused. This is the membership epoch (SI4) made concrete, and S06's two-way ordering test proves both orders.

### 10.5

**One function decides.** `authorize(session, action, resource)` over one permission table, deny by default, on every request, never on the client, and re-checked by the callee. If you look for authorization anywhere else in the code, that is a finding.

**Why signed tokens and not mTLS.** Mutual TLS, where both ends of a connection present certificates, needs a certificate authority someone runs, and nobody here runs one (A8); a signed 5-minute token names its callee, so a captured token replays only there, briefly. The trade is written down as AR3 rather than hidden.

**Operator access as design.** The operator role sees no organization data; reading it is a just-in-time grant an administrator approves for an hour; breakglass is a sealed role behind a hardware key whose trail a named outside person reads; destructive actions take two people. One founder cannot yet staff all of that, and the record says so in AR1 instead of pretending.

### 10.6

**What is unusual in the secrets table.** The platform holds almost none of the secrets a sign-in integration usually brings: its client at fAuth is a public PKCE client with no secret, fAuth's keys are fAuth's, and the only long-lived secret in the whole system is the provider's API key, which is why that key gets its own spend-limited account unit and a 90-day rotation. The service signing keys are the other unusual row: they live an hour in memory and are vouched for by a key-service key that is replaced yearly by hand, because asymmetric keys there do not rotate automatically.

**The one deliberate exception to "fails closed".** Live sessions continue when fAuth or its key set is unreachable, because the token was verified at mint and revocation is the platform's own epoch, not fAuth's. Every other item in that list refuses. [SSE-KMS](https://docs.aws.amazon.com/AmazonS3/latest/userguide/UsingKMSEncryption.html) and [encryption context](https://docs.aws.amazon.com/kms/latest/developerguide/encrypt_context.html) are the two vendor mechanisms the per-organization key relies on.

**Logs never hold a value.** Allowlisted fields only (identifiers, digests, codes, durations, a request id), and filters travel in request bodies because the edge logs URLs. Where data may appear by accident is a design question.

### 10.7

**What "accepted" means.** An accepted risk has an owner (the head), a reason, and usually a condition that closes it: AR4 closes when FORMA-248 moves fAuth's key into KMS, AR1 when a second person exists. Accepting is a decision recorded so it can be revisited, not a risk forgotten. The residual column in 10.4 and appendix D links each row to the AR it rests on.

**Why anchoring every 15 minutes and not every row.** AR9 says it plainly: anchoring every row would put S3 in every transaction, making a soft dependency hard on the commit path. The window a breakglass could alter is bounded, alerted (AL11) and verified daily. Accepting a bounded window beats pretending to an unbounded guarantee.

**The re-run list.** The last paragraph names the events that reopen this section: a new surface, a new assistant tool, a change to admission or the grammar, an upgrade of Forma, fAuth or DuckDB, an incident, the first real organization. A threat model is a living artifact only if its triggers are written down.

### 11

**What is unusual here.** The operator is a trust level with a blast radius (TL5, TH7), not a person who is assumed good: every command in the failure table and every page returns health, jobs, quotas and restore state and never a row of an organization's data. Most operations sections are written for an operator who can see everything; this one is written so that the on-call engineer cannot, and says what they do instead.

### 11.1

**Cold start order.** Alarms are created before anything that should alarm; the workspace server alarms rather than refusing to start without fAuth's key set. Order of bring-up is a design decision because it determines what you can see when things go wrong.

**Callee first, expand then contract.** The data service deploys before the control plane, which deploys before the workspace server, so a caller never meets a callee that does not yet understand it; schema changes add in one release and remove in the next so either build runs against the schema. These two rules are what make the rollback in the next paragraph safe.

**Three waves and one metric.** A tenth of requests, plus every request carrying the canary organization's name (its alias or host, routed whole to the new build so its suite runs against every wave), then half, then all, each baked 30 minutes and 1,000 requests behind a [weighted target group](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/load-balancer-listeners.html). `forma_canary_failures_total` alone rolls a wave back: any error, any cross-tenant hit, any receipt whose result digest differs from the recorded one for the same key. The rollback metric is correctness and isolation, deliberately narrow; a page from 11.3 blocks the next wave instead. [Automating safe, hands-off deployments](https://builder.aws.com/content/3ErTKQOTKc5NIw031UePBPxTQ6I/automating-safe-hands-off-deployments) is the pattern.

**Migration with a bridge and a way back.** Five steps from the demo's one process to the platform, each with the thing that lets both run and the thing that lets you retreat. Notice step 4: the same request types run in process and over the network, which is only possible because D11 made the roles one binary with internal interfaces.

**What the reductions table is for.** Read the right-hand column as a list of interfaces that must already exist in the demo even though the demo does not need them: the organization id on every row with two synthetic organizations, registered renderer versions with two apps, versioned policy rows with one administrator. A reduction without a seam would be a shortcut the platform later pays for; this table is how the demo's lead and this record agree on what may be small and what may not.

### 11.2

**At 10x, by arithmetic.** 600 operations a second at 100 ms is 60 in flight; at 8 a node that is eight data tasks, plus one for a zone loss. Four workspace tasks for 1,200 primary reads a second (two reads a request). The worker count follows A5's caps scaled ten times. PostgreSQL stays one primary, grown a size, because sessions and epochs must stay on it (section 8). Every line here is the 5.2 fit check multiplied by ten.

**Overload has an order.** Record commands and published queries are never shed; generation and explanations go first, then imports, then suggestions, then previews. Writing the order down before the incident is the difference between shedding and failing. "Not in the latency SLIs" means shed requests do not count against the latency indicators (an SLI is the measured quantity an SLO sets a target for), but do count as load for scaling.

**The cost table's driver, and the lines you cannot rebuild.** The data service is $340 of the $565 Fargate line, two tasks of 4 vCPU and 16 GiB, and the second task is for availability, so half of the data service's $340 share, the largest part of the largest line, is bought for N2, which 12.1 counts as a drawback. The load-balancer line cannot be rebuilt from the record alone: "capacity units" are the balancer's usage metric (a blend of connections, bandwidth and rule evaluations) that the vendor prices by the hour, and the record gives only the base rate. Generation is small because retrieval runs first and A5 admits one job per organization; its ceiling, $26,000 with every slot busy all month, is why N10's budget stop exists. [Fargate pricing](https://aws.amazon.com/fargate/pricing/) is the vendor page behind the unit prices.

### 11.3

**SLOs with budgets.** Each objective comes with the budget it allows in a 30-day window and the measurement that spends it; SLO5, revocation, has a zero-miss budget because N7 is never traded; the record files the 5 s bound itself under liveness (SI4, N5), since it promises that a revocation arrives, not that nothing bad happens meanwhile. The policy sentence is the operational teeth: a spent budget stops releases to that path.

**Why 14.4 times.** AL1 pages when SLO1's budget burns at 14.4 times its steady rate for an hour, which is the rate at which 2% of a 30-day budget disappears in that hour. [Alerting on SLOs (SRE workbook)](https://sre.google/workbook/alerting-on-slos/) derives the number.

**Pages versus tickets.** A page means a person is woken: budget burn, a canary failure, commit latency, the revocation probe, a burst of denials, an audit write failure. A ticket waits for the morning: an old queued job, shed rate, storage runway, provider spend. The job alert is on the age of the oldest job, not the queue depth, because a deep queue draining fast is healthy and a shallow one not moving is not ([avoiding insurmountable queue backlogs](https://builder.aws.com/content/3EuRcgkTP1MI0c7zM8W6HL3WIqA/avoiding-insurmountable-queue-backlogs)).

### 11.4

**Read the detection column first.** A failure nobody can see is not mitigated. The row to study is "a wrong number with a 200": a silent failure has no error to alert on, so the design re-derives sampled receipts nightly and quarantines a revision whose numbers no longer match. Receipts make silent errors detectable after the fact.

**RPO promised versus achieved.** N4 promises 24 hours; point-in-time recovery gives 5 minutes in practice ([RDS point-in-time recovery](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_PIT.html)). Promising the weaker number leaves room for the drill to fail a little and still pass. The drill's pass condition is concrete: Maya's case reopens with 1,140,000 bytes recomputed and the cross-tenant suite passes inside 4 hours.

**What is unusual in the edge row and the last row.** A new WAF rule runs in "count mode" first, the firewall's setting that records what a rule would have blocked without blocking it, so a bad rule is seen before it refuses real members. And the two actions with the largest consequences, moving an organization and leaving the region, are explicitly never triggered by failure detection: a person commits them. Automation stops where a wrong automatic decision would be worse than a slow human one.

### 12.1

**What is unusual about this list.** Each drawback is priced in the record's own currency: half of the largest cost line bought for availability, a six-view refresh that meets its own organization's cap before the node's, an audit chain verified daily rather than at write because it has three writers. Those are consequences a reviewer can weigh against the goals, not apologies. And the last item, "the value rests on reuse nobody has measured", concedes the business case, which is rarer than conceding a technical cost.

### 12.2

**What is unusual in these records.** Each rejected option is named by what it is good at, and the chosen one is confirmed by a test that could still prove it wrong (the fifteen-intent suite, the cross-tenant suite, the measured route, the spike's cold aggregate). The "Do nothing" and "Reuse or buy" rows name real products: Retool, Airtable and Glide each run generated code as trusted or make a decision a cell, which is exactly what N7 and G2 forbid; "json-render-style libraries" are the libraries that take a JSON description of a screen and render it, D1's rendering half without any authorization, server arithmetic or publication behind it. The decision-record form itself is [Nygard's](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) with the options made explicit.

**The shared database, named as a choice.** All roles share one PostgreSQL so a publication's record, audit row and pointer commit in one transaction; a database per role would make that distributed. "Tan's framing" is the system design book appendix B cites, in which a boundary between services is first of all a security boundary, a credential that cannot write what another writes; the record adopts that reading and pays the coupling cost it names. [Shared database](https://microservices.io/patterns/data/shared-database.html) is the pattern page.

### D1

**Why a typed definition and not markup.** The question D1 answers is what the model is allowed to produce. Markup (design A's path) can express anything and therefore needs a sanitizer, an admission tier for untrusted compiled modules, a translator and a preview frame, and it kept two High/High threat rows and loaded 28,337 B against a 28,100 B route budget. A typed definition can express only what first-party renderers understand, so the reviewer approves data, a semantic diff is exact, and a hostile proposal fails validation whole. The cost is honest: a widget the vocabulary lacks is an engineering release, and the demo shows assembly rather than novelty (RK6). The custom-widget tier is exactly the rejected option, kept for a customer who needs it.

### D2

**Why the control plane alone writes publications.** The reason is one sentence in the record: the writer of publications must not handle model output. Options L (publish from the workspace server) and M (a signed manifest without a record) each remove a part and lose the separation or the thing adoption compares against. Option N, a WebAuthn step-up on every approval, is kept as a policy option rather than a requirement because it would forbid the owner-publish policy R9 requires. A one-way door, because the publication record is the organization's evidence of what ran.

### D3

**Two independent walls.** Against a query that omits its organization: the id taken from the verified token and the row policy. Against a role that sets the wrong id: the adapter as the id's only source and the canary suite each release. The pool is chosen over a silo because ten databases now and a hundred at N1 is more than one operator can run (A8, N10), and the record accepts the consequence in writing: a policy bug is a cross-organization bug, so the suite runs every release. Cells, a separate pool an organization moves to whole, are the escape hatch for the day the pool is not enough.

### D4

**"Evidence never changes, records always do."** That sentence decides the split: immutable Parquet revisions for sources, because a receipt must be able to point at exactly the bytes forever; PostgreSQL for records, because a form needs a transaction. Options D and E (PostgreSQL tables per dataset, SQLite per revision) each put the wrong workload on an engine. The last sentence is the one to remember: the fallback for a slow cold aggregate changes DuckDB's plan, not the grammar or the receipts, which is why D4 is a one-way door and DuckDB is not.

### 12.3

**Abandoned ideas carry their pitfall.** Public pages and MCP first (proving isolation on the surface with the most threats), the "nothing migrates" promise (a compatibility assertion nobody tested), markup for the first release (a demo waiting on two security-critical tickets), WebAuthn on every approval (a ceremony mistaken for the digest it signs). Recording why an idea was dropped stops the next reader proposing it again.

### 13.1

**The rank scale is borrowed.** Risks use 10.4's impact scale, so a High here and a High there mean the same thing. RK1 is honest to the point of discomfort: no customer, no baseline, no mitigation in this record. RK5, the unmeasured cold aggregate, is Medium rather than High because the fallback keeps receipts and D4 unchanged.

**Notice the debt list.** One region, no mTLS, a client row by hand, two engines, 2.x canvas geometry. Debt named with its AR or D is debt a reviewer can price.

### 13.2

**What is unusual here.** Every question carries the choice the build proceeds with until it is answered, and the thing that answers it is named (the spike, the load test, the fifteen-intent suite, the head), so no question is a reason to wait. Read "Before build" as the two measurements that could still change a door: a cold aggregate over 1.8 s brings pre-aggregation, a route over 28,100 B brings a framework budget; everything else moves a number.

### 13.3

**What the four objections have in common.** Each is the question a particular reader asks first (a product person, a database person, someone who read design A, someone who distrusts the model), and each answer is a pointer into the body rather than a new argument, which is the test of whether the body already did its job. Try your own first question against them; if it is missing it is either a gap or a question only you would ask, and both are worth raising.

### A

**The glossary is merged into this edition.** Every term here appears on the Glossary tab with the terms this layer adds, and the first use of each in every section is linked with the definition on hover. Appendix A repeats the record's inline definitions; this layer defines only what it does not.

### C

**Read the hard rows together.** Edge, workspace server, data service, PostgreSQL, and object storage and key management on the cold path: multiply their commitments and you have 5.2's 99.65% and 99.55%, which bound N2. The last row is easy to miss: the `AppDefinition`, receipt and Parquet formats are a dependency shared by every role and the browser, versioned like the API and checked by a schema-diff test in CI.

### D

**Where to start in the long table.** The notes in sections 4 to 9 kept pointing here; this is why. TH8 to TH11 are the rows whose controls the body already built without naming them: the service token (6.1, 10.5) answers TH8, the DuckDB confinement (6.3) answers TH9, the receipt-digest cache key (6.3, 8) answers TH10, the insert-only chained audit (6.3, 10.6) answers TH11. Read those four first and you will recognise each mechanism; TH12 to TH15 (a stolen cookie, injected instructions, denial of service, an out-of-grammar proposal) are the ones a seat, a file or a sentence is enough to attempt, held inside one organization by the epoch, the five tools, the slots and whole-output validation.

## Glossary

- **C01 to C16** — the shared platform and demo contracts (`SHARED-CONTRACTS.md`), agreed before this record so the demo and the platform decide the same things; the record restates whatever it relies on, so a C number is a citation, not required reading (cover)
- **P01 to P16** — the product requirements of the validation plan (`VALIDATION.md`), the criteria the release is judged on; each R line names the ones it answers and the task map lists them (cover, 4.1)
- **K1 to K5** — the head's kickoff rulings for this task, recorded in the intake; K1 is the ruling for a typed definition over markup, K4 for fAuth behind the adapter (cover, 2)
- **H0 to H11** — the head's earlier decisions on design A; this record names them only where it keeps or drops one (3.2, 13.3)
- **I-1 to I-4** — the gaps the foundation review found in the earlier design; R3, R4, N12 and R4's clarifying question close all four (4.1)
- **L2 | L3** — the factory's document sizes: L3 the full record this is, L2 the smaller record the demo has (cover)
- **identity adapter | adapter** — one small component that talks to the sign-in provider and hands the rest of the platform a verified principal, so the provider can change without the rest noticing (1, D6)
- **mountMarkup | reconcile** — the 3.0.0 runtime's call for mounting markup and the 2.x call the prototypes' widgets use; the prototypes do not mount on 3.0.0 (2)
- **0 FAIL gate** — the framework's own release check with no failing item (2)
- **loopback** — a server that answers only its own machine; the prototypes' state (2)
- **fixture** — fixed synthetic data with known expected numbers that scenarios, tests and drills run against (2, 5)
- **native subset | subset** — the part of TypeScript the compiler accepts for server-rendered components; a computed comparison is refused (BUILD-610), and appendix B names the other diagnostics (2)
- **callee** — a program the platform calls, as opposed to one that calls the platform; in this release every programmatic party is one, so no API client exists (4.1, 10.5)
- **stimulus** — in the six-part quality scenario (source, stimulus, environment, artifact, response, measure), the event the system must respond to (4.2)
- **Little's law** — the number of requests in progress equals the arrival rate times the time each one takes; 60 a second times 0.1 s is 6 in flight (5.2)
- **p50 | median** — the value half of the measurements are below; the typical case (N3)
- **p99 | 99th percentile** — the value 99 of 100 measurements are below; the tail the design is held to (N3)
- **p75** — the 75th percentile, the point at which Core Web Vitals assesses a page (N3)
- **p99.83** — the per-operation percentile that makes six operations together meet p99, since 0.9983 to the sixth power is about 0.99 (N3)
- **RPO | recovery point objective** — how much recent data may be lost in a disaster, measured in time; 24 hours promised, 5 minutes in practice (A7, 11.4)
- **RTO | recovery time objective** — how long service may be down while it is restored; 4 hours, proven by drill (A7)
- **error budget** — the share of a window an objective allows to be missed; 99.5% of a month leaves 3.65 hours (N2, 11.3)
- **golden signals | four golden signals** — latency, traffic, errors and saturation, the four things to measure on any service (11.3)
- **SLO | service level objective** — a target for a measured indicator over a window, with the budget it allows (11.3)
- **SLI | indicator** — the measured quantity an objective sets a target for, such as the p99 of the six-view refresh; shed requests are kept out of the latency ones (11.2, 11.3)
- **real-user telemetry | real-user timing** — latency measured in members' browsers rather than at the server, so it includes the network (N3, 11.3)
- **LCP | Largest Contentful Paint | Core Web Vitals** — the time until the largest element of a page is drawn; one of the browser-measured page quality metrics (N3)
- **first paint** — the moment the server-rendered page is visible, before any JavaScript runs (6.5)
- **WCAG** — the W3C's accessibility guidelines; level AA is the common conformance target (N12)
- **gzip** — the compression the route budget is measured under; 28,100 B gzip is the cap on a route's framework code (2, 6.5)
- **adopt | adopts** — the framework's word for the browser attaching its island code to HTML the server already rendered, what other frameworks call hydration; not adoption of saved work, which appendix A defines (5.2, 6.5)
- **`createResource` | HTML runtime** — a framework data-loading primitive and the framework's module for mounting markup; importing either pulls more framework code into the route, which the 426 B of headroom cannot take (6.5)
- **SSE-KMS** — object storage encrypting each object with a key held in the key management service; here one key per organization (10.6)
- **encryption context** — a name attached to a key operation that the key policy can require, so a key is used only for the organization it belongs to (TH1, 10.2)
- **envelope** — a data key wrapped by a master key; the master key unwraps it one call at a time (6.6, 10.6)
- **SigV4** — the cloud's request signing: every call to object storage or the key service is signed with the task role's credentials and carries the organization's encryption context, so the store and the key policy both check it (10.2)
- **SDK** — the vendor's client library that signs calls and parses replies; "the SDK" validates the cloud's responses in the entry-point table (10.2)
- **per-query S3 session | S3 session** — temporary cloud credentials minted for one query whose policy allows only that organization's prefix, so even a wrong path is refused by the store (6.3, TH1)
- **KMS-certified key | certifying KMS key** — a role's hourly in-memory signing key, whose public half the role's long-lived asymmetric key in the key service has signed; callees trust the key-service key and so accept the hourly one (6.1, 10.5)
- **PKCE** — proof key for code exchange: the client sends a hash before sign-in and the secret after, so a stolen authorization code cannot be exchanged by someone else (SR2)
- **public PKCE client | public client** — an OAuth client that holds no secret because PKCE proves it started the sign-in; the platform's `oauth_clients` row at fAuth is one (10.6, 6.1)
- **authorization code | back channel** — the one-time code the browser carries back from sign-in, exchanged for the token by the server directly, never by browser script (7.2)
- **`state`** — a random value bound to a short-lived cookie that ties a sign-in callback to the browser that started it (6.1)
- **EdDSA** — the signature scheme fAuth signs tokens with; verified with a public key, so the platform needs no secret of fAuth's (SR2)
- **`kid`** — the key identifier in a token's header that says which key of the key set signed it (6.1)
- **`sub` | `tenant_id`** — the token claims naming the person and the organization; the session is minted from these two and nothing else (D6)
- **slug header** — a request header naming an organization by its short name, which fAuth accepts as a fallback for finding a tenant; the platform ignores it and takes the organization from the signed token only (TH1)
- **refresh cookie | refresh-cookie bridge** — fAuth's own cross-site cookie on its origin that could keep a browser signed in; the platform does not use it, so nothing cross-site carries authority (TH3)
- **opaque cookie | `__Host-`** — a session cookie that carries only a random id, with the browser prefix that pins it to one host over HTTPS (6.1)
- **HttpOnly** — a cookie flag that keeps page scripts from reading the cookie (9)
- **SameSite** — a cookie flag that limits when a browser sends the cookie on requests from other sites (TH12)
- **Sec-Fetch-Site** — a header the browser sets saying whether a request came from the same origin; state changes without it or a matching Origin are refused (9)
- **CSP | nonce** — the content security policy that lets a page run only scripts carrying the per-response random value (TH25)
- **cross-site** — a request a hostile site makes to this one with the member's cookie attached; refused as 403 `cross_site` (9)
- **OIDC | OpenID Connect** — the identity protocol over OAuth; here also how the release workflow authenticates to the cloud without a stored key (TH6)
- **OAuth | registered client** — the delegation protocol behind the sign-in handoff; the platform is a client fAuth knows by a row, a public one with no secret (6.1, 10.6)
- **TOTP** — a one-time code from an authenticator app; the operators' second factor today (10.5)
- **hardware key** — a physical security key; required for breakglass and for operators before the first real organization (10.5)
- **replay** — presenting a captured code, token or approval a second time; every flow here has a check that makes the second use fail (TH3, TH5)
- **mTLS** — mutual TLS, where both ends of a connection present certificates; rejected here because nobody runs the certificate authority it needs (10.5)
- **STRIDE** — six threat categories: spoofing, tampering, repudiation, information disclosure, denial of service, elevation of privilege (10.4)
- **trust boundaries | trust boundary** — a line on the data-flow diagram across which trust changes, drawn dashed and lettered B; each crossing gets a threat row (7)
- **DFD | data-flow diagram** — the picture of processes, stores, external entities and boundaries that the threat table cites (7, 10.2)
- **invariant** — a property that must hold however the environment misbehaves, with the components that enforce it named (10.2)
- **trusted computing base** — the components that enforce the invariants; everything else may fail without breaking them (10.2)
- **blast radius** — what a compromise of one boundary can reach, and what it cannot (10.2)
- **threat actor** — who attacks, with their capability and motive (10.3)
- **attack tree** — a goal broken into the ways of reaching it, OR by default, AND where all steps are needed (10.4)
- **residual** — the risk that remains after the mitigations, rated on the same scale (10.4)
- **separation of duties** — no single role or person can both do a thing and approve it; the author never approves her own draft (10.5)
- **non-Byzantine** — a node that may fail or stop but does not lie; the assumption made for nodes inside the VPC (SA1, 10.1)
- **deny by default** — anything not explicitly permitted is refused (10.5)
- **fail closed | fails closed** — when a control cannot decide, it refuses rather than serves (SR12, 10.6)
- **principal** — the verified identity a request acts as, with its organization, roles and epoch (6.1)
- **scope** — the set of data a principal may reach; always the session's organization and never a request value (SI1)
- **data protection impact assessment** — the risk assessment the GDPR requires of a data controller before high-risk processing (10.1)
- **controller | processor** — under data protection law, the party that decides why data is processed and the party that processes it for them; the organization and Forma (10.1)
- **data classes** — Restricted, Confidential and Internal: labels that decide protection and retention per kind of data (10.6)
- **hash chain | hash-chained** — each audit row carries the hash of the one before it, so a deleted or altered row breaks the chain (SR10)
- **anchor** — a chain head written to a store nothing can change, so the chain up to that point cannot be rewritten (10.6)
- **Object Lock** — an object storage setting that makes an object undeletable and unchangeable for a period (TH11)
- **advisory lock** — a PostgreSQL lock on a number the application chooses, used here so two writers never race on a chain's head (6.3)
- **SERIALIZABLE** — the isolation level under which transactions behave as if run one after another; PostgreSQL aborts one that would not (6.2)
- **read committed | READ COMMITTED** — the default isolation level: a statement sees what was committed when it started, so cross-row premises need explicit locks (6.3, 8)
- **write skew** — two transactions each reading a consistent premise and both committing changes that together break it; what SERIALIZABLE prevents (6.2)
- **`FOR UPDATE` | `FOR SHARE`** — row locks: an exclusive one for a row about to change, a shared one that blocks any exclusive lock until the transaction ends (6.3)
- **SKIP LOCKED** — a row-lock option that passes over rows another transaction holds, so many workers claim different jobs (6.4)
- **row lock** — a lock on one row; two commands on one record wait for each other on it (5.2, 8)
- **hot record** — one record many people commit to at once; one row lock, about 50 commits a second (8)
- **cap, not a split** — a record hotter than its row lock allows is throttled (429 past 20 waiting), never divided into several rows (8)
- **lease | claim sequence** — a time-limited claim on a job, renewed while work continues, with a counter that fences a late result (6.4)
- **fenced commit | fence** — a write that cannot land after a newer claim or a revocation because a lock or a sequence number stands in its way (6.3, 8)
- **linearizable** — every operation appears to happen at one instant in a single order; promised per record (N5)
- **read-after-write** — a reader sees its own committed write; the primary gives this, a lagging replica does not (8)
- **monotonic | monotonic read** — a reader never sees an older state after a newer one; with read-after-write, what the primary promises and a replica may not (8)
- **compare-and-set** — change a value only if it still equals what you last saw; a stale attempt changes zero rows and returns 412 (D14, 8)
- **adoption table** — rows the publisher writes at publication, one per earlier publication of the app whose record schema the new one can still read, so adoption is a lookup (6.2, 7.3)
- **idempotency outcome** — the stored result of a command under its key, returned unchanged to any retry (6.3, 9)
- **stale edit** — an edit whose expected version is behind the record's current one; it conflicts into a recovery view, never a silent overwrite (7.3)
- **recovery view** — the authorized current state returned with a 409, so the member can redo the edit on top of it (D5)
- **half-open window | half-open** — an interval that includes its start and excludes its end, written [10:00, 10:10), so adjacent windows never share an event (G3, 7.2)
- **snapshot | record snapshot | commit sequence** — the point in PostgreSQL's history a query's records were read at, named by a counter the receipt stores (6.3)
- **policy epoch | filter epoch** — the policy counter a receipt pins, and the browser's counter per filter change that late pages are compared against (6.5, 8)
- **tombstone** — a small marker left where data was deleted, carrying id, digests, actor and time but no payload (8)
- **soft-delete | hold** — deletion that can be undone for a period; an organization's deletion waits 24 hours and can be cancelled (8, TH28)
- **digest | content digest** — a SHA-256 hash that names a definition, revision, receipt or result by its exact bytes (6.2)
- **semantic diff** — a comparison that shows what changed in meaning (a measure, a filter, a transition), not which characters moved (R5, 6.1)
- **patch | base revision** — a change expressed against a named earlier revision of the definition; a patch on a revision that is no longer current is refused with 412 (R5, D9)
- **whole-output validation** — the proposal is checked complete against the grammar and stored only if all of it passes; nothing partial (D8)
- **grammar | query grammar | syntax tree** — the closed set of typed operations a definition may ask for, compiled to the engine from its parsed form and never from text (6.3)
- **typed query** — a query expressed in the grammar's operations rather than as SQL (D4)
- **retrieval | retrieved | adapted | generated** — the three labels on a creation: found as is, found and changed, or newly proposed (R4)
- **embed | embedding** — turning text into a vector so similar texts are near each other; done in process here, with no provider call (6.4)
- **index file** — one per-organization file of catalog metadata and examples that retrieval searches, rebuilt within 60 s of a publication (D7)
- **injected instruction** — text inside evidence written to steer the model; reaches the provider only as delimited data (TA5, TH13)
- **provider tokens | model tokens** — the units a model provider bills by, in and out; the cost line N10 meters (11.2)
- **catalog metadata** — titles, descriptions, examples and field names of published apps; what retrieval and the provider see instead of rows (D7)
- **DuckDB** — an embedded analytical database that reads Parquet directly; run here as a locked-down instance per operation (6.3)
- **Parquet** — a columnar file format: values of one column stored together, so a query reads only the columns it needs (D4, 8)
- **row group** — a horizontal slice of a Parquet file; sorting by time lets a window skip the groups it does not overlap (8)
- **columnar** — stored column by column rather than row by row; cheap for aggregates over a few columns (12.2)
- **range-read** — fetching only the byte ranges of a file a query needs (7.2)
- **pre-aggregation | aggregates at admission** — computing a result ahead of time when a definition is admitted; the fallback if the cold aggregate is too slow (5.2, D4)
- **cold path | warm** — a query whose revision must first be fetched and decrypted, against one already in the node's cache (5.2)
- **WebAssembly | WebAssembly sandbox** — a portable binary format run by a host that controls exactly which capabilities the code gets (SR5)
- **fuel** — a WebAssembly runtime's budget of instructions, after which the module is stopped (6.4)
- **system interface** — the host functions a WebAssembly module may call; here an input and an output stream only (SR5)
- **SBOM** — a software bill of materials: the list of every component in a build, kept beside the signed image (SR8)
- **keyless signing | Sigstore** — signing a release with a short-lived certificate tied to the workflow's identity instead of a stored key (TH6)
- **lockfile** — the file pinning every dependency to an exact version and checksum (SR8)
- **SHA-pinned | commit SHA** — a CI action referenced by its exact commit rather than a tag that can move (TH6)
- **provenance** — the record of how and from what an artifact was built (SR8)
- **task role** — the cloud identity an ECS task runs as; one per platform role, with its own grants (10.5)
- **IAM authentication** — short-lived database credentials issued to a task role instead of a stored password (10.5)
- **security group** — a cloud firewall rule set that admits only the listed caller (10.5)
- **VPC** — the private network the roles run in; boundary B2 (7)
- **availability zone | zone** — one of a region's independent data centres; the design runs in two (11.1)
- **egress** — traffic leaving the network; only workers have it to the provider, and it is a cost line (6.4, 11.2)
- **NAT gateway** — the managed path that lets private tasks reach the Internet without being reachable from it (11.2)
- **TTL** — how long a DNS answer may be cached; it bounds how fast a failover takes effect (11.4)
- **Route 53** — the DNS service that fails the edge name over on health checks (11.4)
- **Secrets Manager** — the store the provider key lives in, read by the owning task role only (10.6)
- **CloudTrail | CloudWatch** — the cloud's audit log of API calls, and its metrics and logs service (10.5, 11.3)
- **ECS | Fargate** — the container service, and its mode where the cloud runs the hosts (D15)
- **ALB | load balancer** — the layer that terminates TLS and spreads requests across tasks (6.6)
- **capacity units** — the load balancer's usage metric, a blend of connections, bandwidth and rule evaluations, priced by the hour on top of the base rate (11.2)
- **WAF | web ACL** — the rule set in front of the load balancer that blocks hostile traffic patterns (6.6)
- **count mode** — a firewall rule setting that records what the rule would have blocked without blocking it; every new rule runs in it first (11.4)
- **CloudFront** — the content delivery network that serves hash-named assets near the member (6.6)
- **S3 | object storage** — the store for immutable files named by key; here originals, Parquet, exports and index files (6.6)
- **RDS** — the managed PostgreSQL service; Multi-AZ means a synchronous standby in a second zone (6.6)
- **KMS** — the key management service holding one data key per organization and the platform's keys (6.6)
- **synchronous standby | Multi-AZ** — a second database copy that must have the bytes before a commit is acknowledged (6.6)
- **failover** — the standby taking over as primary, 60 to 120 seconds here (11.4)
- **read replica** — an asynchronous copy for reads that may lag; added at 10x, never for sessions or epochs (11.2)
- **point-in-time recovery** — restoring a database to any moment within the backup window, 5-minute granularity here (11.4)
- **versioning** — object storage keeping previous versions of an object, so an overwrite or delete can be undone (8)
- **gp3 | vCPU | GiB | GB** — a storage class, a unit of compute and the two sizes of a gigabyte: GiB is 1,024 cubed bytes, GB is 1,000 cubed; the cost table uses both (11.2)
- **readiness probe | liveness probe** — the health check that says a task may receive traffic, and the one that says its process is alive (11.1)
- **weighted target group** — a load-balancer rule sending a chosen share of requests to the new build (11.1)
- **canary name** — the canary organization's alias or host on a request, which the rollout routes whole to the new build so the canary's suite runs against every wave (11.1)
- **bake | baked** — the time a wave runs under watch before the next wave starts (11.1)
- **callee first | callee-first** — deploying the service that is called before the service that calls it (11.1)
- **expand and contract | expand** — a schema change made in two releases: add the new shape, then remove the old once nothing uses it (11.1)
- **drain | deregister** — stopping new requests to a task and letting the in-flight ones finish before it stops (11.1)
- **schema-diff test** — a CI test comparing the new API or format against the previous version's fixtures (6.7, 9)
- **`Retry-After`** — the response header saying when to try again; the only thing the browser retries on (9)
- **`Deprecation` | `Sunset`** — headers announcing that an endpoint will go and when (9)
- **problem+json | problem details** — the standard JSON shape for an HTTP error, with a type and a stable code (9)
- **compatibility | backward compatible | forward compatible** — old requests still accepted, new responses still readable by old clients (9)
- **tags** — the identifiers fields carry on the wire in the API and the shared formats; a removed field's tag is never given to a new field, so an old reader cannot misread it (9)
- **load shedding | shed** — refusing low-priority work under overload so that high-priority work keeps its latency (11.2)
- **criticality** — the order in which work is shed: record commands and published queries last (11.2)
- **open-loop load** — a load test that sends requests at a fixed rate whatever the system's response time, so a slowing system is not spared (N1)
- **burn | budget burning** — spending an error budget faster than its steady rate; the signal AL1 pages on (11.3)
- **runway** — the days until a store is full at the current growth (11.3)
- **quarantine** — taking a revision out of use when its re-derived numbers no longer match its receipts (11.4)
- **re-derivation** — recomputing a cited result from its receipt to check it still matches (SR7, 11.4)
- **split brain** — two nodes both believing they are the primary; ruled out by one writer by name (11.4)
- **blameless postmortem | postmortem** — an incident write-up that names causes and actions, not people (11.4)
- **drill** — a rehearsed failure or restore run on purpose to prove a mitigation (11.4)
- **isolated environment | isolated account** — a separate cloud account a restore is rehearsed into, so the drill touches no live data (N4)
- **jitter** — a random delay added to a retry so many clients do not retry at the same instant (6.4)
- **pooled tenancy | pooled | pool** — many organizations' rows in one database and one schema, kept apart by keys and policies (1, D3)
- **silo** — a separate database and key per organization; the rejected alternative to the pool (12.2)
- **one-way door | two-way door** — a decision expensive to reverse because stored data is in its shape, against one that can be changed later (5.3)
- **fit check** — placing the requirements' numbers on the components that must carry them (5.2)
- **mechanism** — a failure told as the step, the component and the number, rather than as an adjective (2)
- **seam | reduction** — a way the demo is smaller than the platform, and the interface that stays so the full version can replace it (11.1)
- **bridge | way back** — in a migration step, what lets old and new run together, and what lets you retreat (11.1)
- **ephemeral disk** — a task's local disk, wiped when the task stops; the revision cache lives there (10.6)
- **B-tree | partial index | local** — the ordinary database index, one that covers only rows matching a condition (queued jobs), and "local": one index per unpartitioned table rather than one per partition (8)
- **MIT | Apache-2.0 | BSD | MPL-2.0** — the permissive open-source licences dependencies may carry, checked in CI (2)
- **SHA-256 | ECDSA P-256 | AES-256-GCM** — the hash, the signature curve and the authenticated cipher the record names for digests, service tokens and envelopes (10.6)
- **rotation** — replacing a key or secret on a schedule or on suspicion, with the old one revoked (10.6)

## Exercises

### After 1

The summary says D15's region door closes per organization. Before reading 5.3, predict what an organization that wants to move to another region a year in would have to move, and what it could leave behind. Then, when you reach 10.7, find the accepted risk that says why its backups do not shorten the list.

**Answer.** Everything made of the four one-way doors: its originals and Parquet revisions under its key, its PostgreSQL rows (records, work, publications, audit), the key itself, and the receipts that pin all of them; derived data (index files, caches, previews) can be left and rebuilt. Its backups stay in the region (AR10 in 10.7), so they are part of what moves, not a shortcut (1, 5.3, 8, 10.7).

### After 2

Suppose Maya's sheet had shown 228,000 with the label "bytes per minute over the observed events". The number is now honestly labelled. Argue which of section 2's six failures that label removes and which remain, then say what the design does that a well-labelled sheet cannot.

**Answer.** It removes only the missing denominator in step 3, and only for a reader who notices the label. Steps 1, 2, 4, 5 and 6 remain: no source revision, a match rule in Maya's head, rows leaving the company, a verdict without its window and inventory revision, and an overwritable cell. The design binds the denominator into the definition (a 10-minute window) and the receipt pins it with the revisions and the rule, so a later reader cannot get 228,000 from the same app (2, R6, G3).

### After 3.3

A3 budgets the normalized copy at twice the originals. Predict which direction A3's 43.95 GiB moves if the spike measures Parquet revisions smaller than their CSV originals, say why the record budgets high anyway, and name the two numbers in section 11 that would shrink with it.

**Answer.** Down: the 29.3 GiB of Parquet is two thirds of the total, so a Parquet copy at the original's size would bring the 90-day figure to about 29.3 GiB. The record budgets twice "until measured" because only measured numbers are facts and an over-estimate errs toward spare disk. The data tasks' 50 GiB disk in 11.2 and the S3 line (60 GiB) would shrink (A3, 8, 11.2).

### After 4.2

A15's peak factor of 3.3 assumes every seat active at once. Rebuild the design point at a peak of 2 times average instead, carry it into 5.2's in-flight number and N1's 10x row, and say which numbers do not move.

**Answer.** 18 operations a second times 2 is 36 at peak; at 100 ms that is 3.6 in flight, well inside one node's 8 slots; at 10x, 360 a second is 36 in flight, five data tasks plus one for a zone loss instead of eight plus one. The per-organization cap of 2, the p99.83 per operation, the contracts and every safety property do not move, which is exactly what A15's "if wrong" clause says (4.2, A15, 5.2, 11.2).

### After 5.2

Little's law gives 6 in flight at 100 ms. Suppose the spike measures a warm operation at 200 ms. Recompute, and say what breaks at the design point and what still holds.

**Answer.** 60 a second times 0.2 s is 12 in flight, above the node's 8 slots, so the design point no longer fits one node and operations queue (429 past 20 waiting an organization). The per-organization cap of 2 makes a six-view refresh three waves of 200 ms, about 600 ms warm, still inside N3's 2 s, and nothing about isolation or receipts changes. Section 13.2's "a third task and a cap of 6" is the record's own prepared answer (5.2, 6.3, 13.2).

### After 5.3

D4 is a one-way door and DuckDB is not, though DuckDB is the engine that reads D4's Parquet. Explain the difference, using what a receipt pins.

**Answer.** A receipt pins revision digests, operator versions, parameters and a record snapshot, never an engine or a plan. Swapping DuckDB, or precomputing aggregates at admission, changes how a result is produced, and the re-derivation must still match; the Parquet layout and the PostgreSQL rows are what every receipt and every case is made of, so changing them migrates every organization (D4, 6.3, 12.2's D4 record).

### After 6.1

A colleague proposes caching the membership row for 30 s in the workspace server to save the 60 reads a second at peak. Name the requirement row the change breaks and the property the record pays for instead, and say what the proof of that property becomes with the cache.

**Answer.** N3's revocation row: an open display denied within 5 s, and "revocation proved by a read". With a 30 s cache a removed member keeps reading for up to 30 s and the proof becomes a staleness bound you argue about instead of a read you can test. The record spends two primary reads a request, 120 a second at peak, to keep it a read; section 10.2 later names the property as the fourth invariant (6.1, N3, 8).

### After 6.2

Make the case, in two sentences, for letting the workspace server write the publication under the author's session (alternative L), then say what the record protects by refusing it.

**Answer.** For: one fewer role to run, and one fewer hop on the publish path for a founder who runs everything alone. Against: the workspace server is the role that handles the member's request and forwards model output, so a flaw there could insert a publication nobody approved; D2 keeps the only publication writer one database grant away from model output, and SR3's test proves `workspace` cannot insert one (6.2, D2, 12.2).

### After 6.3

Swap the first two steps of the record command: check the expected version before looking up the idempotency outcome. Trace the lost-response retry of `ik-1` and say what the member sees.

**Answer.** The first attempt committed version 1. The retry carries expected version 0; a version check first finds version 1 and returns 409 with a recovery view, so the member believes the decision failed and may submit it again under a new key, producing a second mutation or a confusing conflict. Outcome first finds `ik-1` and returns the original outcome, one mutation (6.3, 7.3 messages 9 to 14, 9).

### After 6.4

Predict what a successful exploit of the XLSX parser can reach in this design, then what it could reach if the parser ran as ordinary code inside the worker process.

**Answer.** In the sandbox: one job's bytes and two streams; no socket, file or credentials endpoint, so the job fails visibly and nothing else is touched. In the worker process: the provider key, the ability to upload under any organization's key, and every revision the worker reads; 10.4 rates that row High on both scales before the sandbox (6.4, SR5).

### After 7.1

The provider returns a proposal with seven views that all parse and an eighth whose kind, "heatmap", no renderer is registered for. Predict what is stored, what Maya sees and what the slot does. Then change it: the eighth view is valid but the proposal has 25 views.

**Answer.** Both are refused whole. Admission checks every kind against a registered renderer and names the first check that failed; an unknown kind fails and nothing falls back, so no partial seven-view app exists, the draft is not admitted and the generation slot frees. Twenty-five views fails the complexity limit (at most 24) the same way. Maya sees the refusal with its reason, and R4 says the composer names the missing capability rather than inventing one (7.1 step 4, 6.2, R4, A5).

### After 7.2

Jordan downloads an export at 10:05; at 10:06 the administrator removes his grant while his canvas is open and one of his queries is still queued. Predict what the platform does about each of the three, and name the sentence in R11 that keeps the first honest.

**Answer.** The export is gone: delivered data is stated as unrecallable, and the record says so rather than pretending to a remote wipe. The canvas is denied within 5 s, because it polls the epoch every 5 s and every request reauthorizes. The queued query is re-checked at execution and again before release and returns denied, nothing cached or shown; R11's "revocation stops future results, queued ones included" is the rule, and "delivered data is stated as unrecallable" is the honest half (7.2, R11, 6.5).

### After 7.3

Two practitioners submit `needs_context` on EXP-002 at the same instant, both with expected version 0, with keys `ik-1` and `ik-9`. Walk the transaction for each in 6.3's lock order and say what each sees. Then say what changes if both had sent the key `ik-1`.

**Answer.** Both insert their outcome rows (different keys, no clash); both take the record row `FOR UPDATE`; one wins, finds the record absent, commits version 1. The other waits on the row lock, then finds version 1 where it expected 0 and returns 409 with the recovery view: one commit, one visible conflict, never last-writer-wins. With the same key, nothing changes between two principals, because the key is scoped to the principal; for one principal retrying, the second insert waits on the outcome row and returns the stored outcome, one mutation (7.3, 6.3, 9).

### After 8

A result cache keyed by (view, parameters) alone would be fast and simple. Name the two guarantees from section 4 it breaks, give one concrete wrong answer it would serve, and say what the record keys the cache by instead.

**Answer.** N7 (nothing of one organization reaches another): the canary with the same view and parameters would receive Northstar's total. And N3's and N5's revocation bound: a result computed for Jordan before his grant was removed would be served after it. The key is the receipt digest, which carries the organization, revisions, record sequence, parameters and policy epoch, and a hit is still re-authorized before release; 10.2 later names the two properties as its first and fourth invariants (8, 6.3, N7, N5).

### After 9

Argue for returning 403 rather than 404 to a member of another organization who opens Northstar's work id (it is more honest, and clients can show a better message). Then say what property of the error model it would give up, and why the one 403 the record does return is safe.

**Answer.** A 403 tells the caller the id exists and belongs to someone, which is a one-bit leak per guess and, across many guesses, a map of another organization's work; the record's rule is that denied, foreign and absent are indistinguishable. The author's own-approval 403 is safe because she already sees the app, so no existence is revealed, and naming the rule tells her what to do (9, R11, 7.3's canary case).

### After 10.2

The island drops any page whose epoch is behind, and polls the membership epoch every 5 s, yet it is excluded from the trusted computing base. Argue why that is correct.

**Answer.** The island runs on the member's machine, where TA3 already holds everything it holds, so nothing it enforces can be relied on. The server re-checks membership at execution and before release, and a query carries its epoch so the server, not the island, can refuse a stale one; the island's discard improves what the member sees, it does not decide what they may see (10.2, SI4, 7.2).

### After 10.4

Rewrite TH5 for a design where an approval is "approve draft 7" with no digest. Which prevent control vanishes, what sequence does the attacker run, and which of the remaining controls still holds?

**Answer.** M10's digest binding vanishes. Maya asks Sarah to approve draft 7, Sarah approves, Maya patches the denominator into revision 8, and the publisher publishes "draft 7's current content", which Sarah never saw. The CHECK that approver is not author still holds, and so does the epoch check, but neither is about content; G2's "a stale approval is refused" becomes impossible to test (TH5, 6.2, G2).

### After 10.5

A service token minted by the workspace server for Maya at epoch 7, callee the data service, is captured inside the VPC at 10:00:00. List what an attacker can do with it and until when, then what they cannot do and which fact stops each attempt.

**Answer.** They can call the data service as Maya, within her organization and her permissions, until 10:05:00, which is what AR3 accepts. They cannot call the control plane (the token names its callee), publish or change policy (the data service holds no such grant), act after an administrator revokes Maya (the callee re-reads the membership row), or act after 10:05 (expiry); and a replaced task rotates the signing keys (10.5, SR3, AR3, 10.6).

### After 10.7

AR2 accepts that a compromised data service can read every organization's revisions. Design the control that would remove AR2, cost it against A8 and N10, and say whether the per-organization keys already limit the exposure.

**Answer.** The control is a data task (or a whole cell, or a silo) per organization, so one credential unwraps one key. The cost is what D3's record names: ten stacks now and a hundred at N1 for one operator, and a second data task per organization in a cost table already dominated by the data service. The keys do not limit it, because the data service's role legitimately unwraps every organization's key with its context; what bounds AR2 is detection, the canary suite each release, and the escape hatch, moving one organization to a cell when it matters (AR2, D3, 11.2, 12.2).

### After 11.1

Wave one of a release doubles the six-view refresh's p99 from 400 ms to 900 ms, with no errors and no receipt mismatches. Using only 11.1, say whether the wave rolls back and what else in 11.1 can stop a release, then explain why the rollback metric was made that narrow.

**Answer.** No: `forma_canary_failures_total` alone rolls a wave back, and it counts errors, cross-tenant hits and receipt mismatches, not latency. 11.1's other brake is "any page of 11.3 blocks the next wave", and 900 ms is inside N3's 2 s, so no page fires; the regression is caught over the month by the refresh objective's budget. The metric is isolation and correctness because those are the failures a rollback must never wait on a human for; latency has budgets and pages (11.1, N3, 11.3).

### After 11.2

Rebuild the Fargate line from the sizes and unit prices, with disk at $0.000111 per GiB-hour and only the data tasks carrying the 50 GiB of disk the record sizes, then the data service's share, then the number of data tasks at 10x.

**Answer.** 13 vCPU × 730 h × $0.04048 is about $384 and 54 GiB × 730 × $0.004445 about $175, together $559; two data tasks × 50 GiB × 730 × $0.000111 add about $8, so about $565. The data service's two tasks are 8 vCPU and 32 GiB, about $236 plus $104, the "$340" the record names, plus their disk. At 10x, 600 a second × 0.1 s is 60 in flight, at 8 a node eight tasks, plus one for a zone loss (11.2, 5.2).

### After 11.3

AL6, the oldest queued job over 10 minutes, is a ticket; AL11, an audit or anchor write failure, is a page. Argue the split from the properties each protects.

**Answer.** A late job is a liveness delay on a cold path the member already sees as queued within 1 s; nothing is wrong about what they see, so it can wait for morning. A failed audit write means actions are happening without the append-only record SI5 promises, a safety property, and the longer it runs the larger the gap; that is worth waking someone (11.3, SI5, N3).

### After 12.2

Option B under D1, both representations at once, is dismissed in one sentence. Make its best case for the demo, then show which of G2's done-when clauses it strains and why the deferred custom-widget tier is not the same thing as B.

**Answer.** For B: the generation moment could show a bespoke widget where the typed vocabulary shows assembly (RK6), and an app could use the best fit per view. G2 requires a publication to bind exact definition and renderer identities and a reviewer to approve exactly what runs; with two representations the reviewer approves two kinds of artifact through two admission paths, one of them needing a sanitizer, which is what "doubles what a reviewer approves" means, and N7 pays for the second path. The custom-widget tier is the markup path deferred behind its own tickets and its own threat model, not a second path in this release's approval flow (12.2, D1, G2, 3.2).

### After 13.1

RK5, the unmeasured cold aggregate, is Medium. Argue for High, then say why the record's rank holds.

**Answer.** For High: the six-view refresh at 2 s is N3's critical row and a miss touches every published app. The rank holds because the fallback, aggregates computed at admission, changes DuckDB's plan and not the grammar, the receipts or D4, so no one-way door and no isolation property is at stake, which is 13.1's definition of High (13.1, 12.2's D4 record, 5.2).

### After C

The two 99.9% factors for the platform's own roles are the record's assumption, not a vendor figure. Argue whether two tasks in two zones justify it and what measurement would replace it; then compose the warm and cold chains from appendix C's hard rows and say why one full restore does not fit N2's month.

**Answer.** Two tasks in two zones means a zone loss or one task's crash leaves a task sized to carry the design point, so the figure assumes the remaining failures are deploys and bugs; only the failure-mode drills and a quarter of real-user availability data can replace it. Warm: 0.999 (CloudFront) × 0.9995 (RDS) × 0.999 (workspace) × 0.999 (data) is about 0.9965; cold adds S3 at 0.999 and KMS at 0.99999, about 0.9955. 99.5% of 730 hours allows 3.65 hours, 219 minutes; a 4-hour restore is 240, 21 minutes over, so N2 assumes no full restore in a month (C, 5.2, N2, 11.4).

## Task map

The controlling definition is `input/context/forma-app-platform-product-definition.pdf`, cited in the record as "the definition, §N"; the evaluation criteria P01 to P16 are `VALIDATION.md`'s product requirements, each bound to shared contracts C01 to C16.

| Ask in the brief | Where the record answers it | What to learn from it |
|---|---|---|
| The definition §9: creation must produce new working behavior, not only a title or a theme | R4, R5, D1, D9, 6.2 (admission), 7.1 steps 3 to 5, G1 | A product sentence becomes a labelled creation (retrieved, adapted, generated) with a semantic diff and an admitted digest, so "new" is something a test can show. |
| The definition §15: no arbitrary untrusted module may run | D1, 3.2 (arbitrary code or markup generation), SI2, TH15, TH29, 12.2's D1 record | The safest way to admit nothing untrusted is a grammar that cannot express it; what the model may produce decides the whole threat model. |
| The definition §16: the six acts of the Northstar demo on one build | 1, G1, 5 (the scenario), 11.1 (the reductions table), C16 in 5.3's "Binds demo" column | Write the demo as the worked scenario and bind the demo's design to the platform's doors, so the small version grows into the large one. |
| The definition §18: the completeness test, two apps through one path with no platform code between | G1, R14, N9, 4.2's change scenario, 11.1 (seams), 12.1 | Generality is observable only as an absence: no branch on a tenant, no commit between two apps; design the test before the feature. |
| The definition §12: adoption of saved work preserves meaning | D14, R10, 6.1 (adoption by compare-and-set), 6.2 (the adoption table), 7.3 (version adoption), 12.2's alternative S | A pinned version and an explicit, atomic adoption keep earlier findings true; automatic upgrade is the alternative that rewrites history. |
| The definition §22: a connector follows demand | 3.2 (live connectors), D12, 13.2 (later), 12.2's alternative Q | Name the interface a later feature will use (sources read-only, commands declared) so deferring it costs no door. |
| P01 Platform, not product: spaces and labels configurable | G1, R14, D3, 8 (spaces hold apps), N9 | A platform is proved by two different apps on one unchanged build, which is a build digest, not a slogan. |
| P02 Predictable authorized navigation | R1, 6.1 (navigator), 6.5 (filter epoch), D7 | Suggestions are filtered before ranking and late results are discarded by epoch; navigation is an authorization surface. |
| P03 App and saved work are separate | R2, R11, D14, 7.2 steps 5 and 6, 8 (saved work) | Three separate grants, app, work and export, each revocable, keep "continue" from copying anything of the author's. |
| P04 Creation includes retrieval, adaptation and genuine composition | R4, D7, D8, 6.4 (composer), 7.1 step 3 | Retrieval first is cheaper and safer than generation; labelling which happened keeps the AI story honest (G6). |
| P05 CSV and bounded XLSX are inspectable inputs | R3, A4, SR5, 6.4 (parser sandbox), 7.1 steps 1 and 2, TH4 | Treat every dropped file as an untrusted author (TL6): caps, a sandbox and confirmed mappings before a revision exists. |
| P08 Draft preview and transformation are contained | R5, D9, 6.1 (draft editor), 7.1 step 4, TH23 | Previews run on synthetic data and draft writes land in an isolated store, so experimenting cannot leak or pollute live work. |
| P06 Queries are correct and bounded | R6, G3, N6, D4, 6.3 (query engine), 7.2 steps 3 and 4 | Compute server-side over the full authorized input and attach a receipt; a number without its denominator and sources is not a number. |
| P07 Forms and internal records support real work | R8, G4, N5, D5, 6.3 (record store), 7.3 and its sequence diagram | One transaction with a fixed lock order gives idempotency, versions, audit and revocation together; add a cache or a second store and one of them breaks. |
| P09 Publication follows organization policy | R9, G2, D2, 6.2, 7.1 step 6, TH5, SR6 | An approval is a session over a digest inside a SERIALIZABLE transaction; policy, never the author or the model, decides. |
| P10 Version changes preserve meaning | R10, D14, 6.2 (the adoption table), 7.3 (version adoption), 8 (publication and adoption pointers) | Compare-and-set on a pointer is how two adopters get one adoption and one 412. |
| P11 Access governs every information surface | G5, N7, D3, SI1, SI4, TH1, TH10, TH18, TH20, 7.2 step 8, 10.4's attack tree | List every surface a result can leave by (search, cache, export, link, queued job) and put a re-check on each; the canary suite proves the list. |
| P12 AI assists within declared authority | R12, G6, D13, SR4, N8, 6.4 (five tools), TH13, TH19 | Give the model identifiers and schemas, never rows or a submit button; the user submits every mutation. |
| P13 Published work survives model outage | R13, N2, D8, 6.7 (soft rows), 4.2's fault scenario | Put the provider off the published-app path by making generation a job; soft dependencies are a design, not luck. |
| P14 Work survives operational failures | N4, A7, SR9, 8 (retention and deletion), 11.4 (failure modes and recovery), A5 | Durable outcomes, a restore drill with a concrete pass condition and visible job state are what "survives" means. |
| P15 Platform generality is observable | G1, R14, N9, 4.2's change scenario, 11.1's reductions table | The proof is a second app from primitives the first never used, with empty revision and deployment logs between them. |
| P16 Canvas and accessible import remain central | R7, R3, N12, D10, 6.5, 5 step 2 | A keyboard path and a focused view must complete the same tasks; accessibility is a requirement with a release check, not a finish. |

## Topics

- design-docs
- decisions
- multi-tenancy
- data
- threat-modeling
- authentication
- processing
- operations
- requirements
- supply-chain
- frontend
- diagrams
