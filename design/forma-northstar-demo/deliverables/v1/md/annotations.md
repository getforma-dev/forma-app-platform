# Annotations: forma-northstar-demo

The per-design layer of the annotated edition (`GUIDE.md` section 8). The method layer, what
each section is for and the course, is `reference/TEACHING.md` and needs no work per task.
This file explains this design to someone learning: why it chose what it chose, the words it
uses, the questions to try before reading on, and where the brief's asks are answered. It adds
no design fact: everything here explains the record, the Northstar demo of the Forma company app
platform, written as a reduced deployment of the platform record; where a note needs one of the
platform's mechanisms it says "platform" and names the section. The gate checks that every key
below points at a section or an identifier that exists (M20) and that every link resolves (M21).

Built with `npm run annotate -- forma-northstar-demo` into `output/design-annotated.html` (plus
`output/flashcards.tsv` for Anki). Keys: a section number (`5.2`, `10.4`), or an identifier the
record defines (`D9`, `TH15`, `SR2`, `N2`). A note keyed to an identifier sits just before the
list, table or paragraph that defines it.

Reader: a developer with two years' experience who has worked through the platform's annotated edition, is reading a second design document, and will build this demo next.

## Notes

### 1

**Before you start: whose identifier is it?** The reading note above the summary sorts every identifier into two kinds. A bare one, such as D5, G6, TH15 or SR2, belongs to this record: it is defined here, and it is what you will build. One after the word "platform", as in "platform D5", "the platform's A4" or "Platform SR1, SR2", belongs to the platform record and is only cited; the demo inherits it unchanged. So does every number in a table column whose heading names the platform: "The platform's" in 5.3, "Seam, the platform's" in 6.8. This edition leaves the parent's identifiers unlinked, so a link always leads to a definition in this record. The two numberings are independent, so one number can mean two things: D5 here is the component library the demo ships with, platform D5 is the record command; D1 here is one process, platform D1 is the typed app definition. Four families continue the platform's numbers instead of restarting, TL, SI, AS and TA, so TL9 is the first trust level this record adds and TA11 the first threat actor; TL0 to TL8 and TA1 to TA10, cited bare, are the platform's. The notes on 10.2 and 10.3 say what each of the platform's is. The rest of the reading note points at documents beside this one: K1 to K5 the head's rulings, C01 to C16 the shared contracts, S01 to S07 the fixtures in `SCENARIOS.md`, and §16, §18 and §19 sections of the controlling definition.

**A dependent record.** This record writes only what differs from the platform record and cites the rest, so that each fact has one home and the two documents cannot drift apart. It keeps without restating the platform's contracts, its decisions platform D1 to D14, its component cards, its flows and its data model, and writes in full what is new: one process instead of four deployments, the eight reductions, the operator's three tools (the provisioning script and the evidence recorder, which run on the operator's machine, and the reset, the binary's one demo-only part, 6.7), the evidence packet, and the threat rows whose control is the demo's own. The price is that some mechanisms are explained only in the platform record, which is why each borrowed sentence names its platform section. **Pitfall.** Reading "the demo" as a smaller, separate program. It is the platform's own code run as one process on one host, plus one demo-only route behind a flag; "keeps the platform's code" in the summary is literal, and it is what lets the demo grow into the platform (G7).

**Notice what the summary does with its weakest point.** One host with no standby is the design's largest cost, and the summary states it plainly: "a restart that is a scene". The design does not hide the restart; it films it and times it against a 30-second limit (N2, in 4.2), so the cost becomes a measurement a partner can weigh instead of the first question a partner asks.

### 2

**Why the problem is told as three shortcuts.** Nothing is built yet, so there is no broken system to describe; the failure this section describes is a recording that looks right and proves nothing. Each shortcut is the cheapest way to make one act look done, and each names what it leaves untested: the denominator lives in code, the decision lives in the browser, one session plays three people. That is the "problem as a mechanism" rule applied to a demo: what fails, at which step, and why a video cannot show it. The design answers all three the same way, with evidence kept outside the video: the packet (S07).

**What a reduction is, and what it is not.** The constraints end with four things that are not reductions: a login dropdown, work kept only in browser memory, faked commits and a hardcoded act 6. The test that separates them: a reduction makes something smaller (fewer organizations, smaller files, a script instead of a page) and keeps the platform's interface for it, the seam through which the platform's mechanism will later replace it; each of the four replaces a mechanism with its appearance, so there is no interface to grow back into. Apply the same test to any shortcut you are tempted by while building.

**Read "What exists" as your starting line.** Three things are [built] at pins you can check out: Forma 3.0.0, fAuth and the composition fixture. Everything else in this record is [proposed], still to be written. Keep the fixture's 27,674 of 28,100 B in mind, 28,100 B being the most compressed Forma code one route, one page, may load; the D5 note explains what the 426 B left can and cannot pay for.

### 3.1

**A goal that can fail.** Each "done when" names a fixture value or an observable absence: G3's 210,000 bytes beside Maya's untouched 1,140,000, G4's version 1 and then version 2, G2's runtime with no branch on `Northstar`, `Security` or `Finance`. Each goal is therefore a test that can come out red, which is what separates a goal from a hope. G2 is the hardest kind to prove, because generality shows only as an absence: a search of the runtime that finds none of those names, and a deploy log with nothing in it between the two apps.

**G7 is about the demo's afterlife.** The other goals are about the recording; G7 asks that every container map onto a platform container, so that the demo is the platform's first deployment rather than a prototype to throw away. It is the goal behind every "seam" you will meet from here on.

### G6

**Three labels, and why a label is not an apology.** A step is seeded when its state was put in place rather than produced on camera (the reset restoring a captured state between takes, a take being one recording of an act or of the whole run; or a fixture proposal standing in for the model's, as in the spike, the measurement run before any scene), simulated when the script provokes or fakes a behavior (act 6's response dropped at the browser to force a retry), and manual when a person does by hand what the product would do. A labeled step may appear on screen, but it is excluded from acceptance: it never counts as evidence that a capability works. 13.3's "What is simulated?" is the full list of the demo's labeled steps, by label. Labels are what let the demo show a hard case honestly, a lost response on camera, without pretending the network failed by chance. **Pitfall.** Treating labels as bookkeeping to finish later. Once YC holds the packet nothing can be relabeled (D7), so an unlabeled seeded step voids the claim it touches.

### 3.2

**Each non-goal is something a viewer might ask for.** A partner could expect a passkey sign-in, a live co-editing cursor, or the MCP scene the earlier demo had, and each line says why the recording does without it. The passkey button would spend act 1 on the identity service's feature instead of the platform's. Live co-editing is one of the demo's reductions (6.3, 6.8), and the property that matters, that no edit is silently lost, is shown in act 6 more cheaply: every edit carries the version it was based on, its expected version, so a stale edit is refused as a visible conflict instead of overwriting. **Notice** "Platform 3.2's non-goals hold here": a demo cannot be larger than its platform, so it inherits every refusal and adds only its own.

### 3.3

**Read each "If wrong" as a map of what moves.** The useful clauses are the narrow ones: A12's "only act 1 changes" says a sign-in problem stays inside the first act, and A9's "G7's seams are exercised" says a second host would test the very interfaces the reductions keep. A clause that said "redesign" would tell a reviewer nothing. A11 states the whole dependency on the platform in one line: if a platform decision moves, this record follows it.

**One Chrome profile an actor (A13).** A profile keeps its own history, passwords, settings and accounts apart from the other profiles on the machine ([Chrome profiles](https://support.google.com/chrome/answer/2364824)), so Maya, Sarah and Jordan can each hold a real, separate session on the one recording machine. It is the direct answer to section 2's third shortcut, one session playing three people.

### A6

**Why caps far above the fixtures.** The fixtures are a six-row CSV and a two-row XLSX, with Jordan's two-row CSV in act 5, so a cap set just above them would refuse almost any real file and say nothing about the parser. A6 sets limits between the fixtures and the platform's own, so that the one refusal act 2 captures (S06) comes from the bounded parser's real limits rather than from a cap tuned to the fixture. The limits are a reduction with a seam: the same pipeline, raised when measured (6.4).

### 4

**The canary is a stakeholder.** The table gives a row to a hostile principal, the canary, whose concern is that every replay is denied and audited. Writing the attacker's experience as a requirement turns isolation into something a test checks on every take, rather than a property you hope holds. Notice who is absent: a customer, because the data is synthetic and a pilot is a non-goal (3.2).

### 4.1

**How to read an R line.** Each line says what must hold, then three kinds of citation in brackets: the act and fixture where the recording shows it (act 6, S05), the platform requirements it inherits (platform R8), and the shared contracts (C07). MUST and MAY carry their [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119) meanings: a MUST that fails is a defect, a MAY is permission. The paragraph above the list already holds the rule section 5 builds on: "a take has one actor at a time".

### R7

**One policy per organization.** R7, below, has Sarah approve Spend Review "under the organization's one policy". Security and Finance are spaces, areas of apps inside one organization, and publication policy belongs to the organization, not to a space or an app (platform R9). So Spend Review inherits Northstar's independent review, and Sarah's approval is ten seconds of act 6. **Pitfall.** Reading a space as a wall between tenants. The wall between companies is the organization, and the canary is another one; inside one organization, grants decide who reaches which space, which is why "a security-only principal cannot read Finance" is tested on its own (7.2).

### R8

**Why a denial needs its owner's success.** A 404 alone cannot tell "refused" from "no such thing". Pairing each replay with the owner's 200 on the same identifier in the same take is what turns the canary's 404 into evidence of a refusal: the identifier was real and live when the canary was turned away. The canary gets 404 rather than 403 so that a refusal never confirms that the thing exists; HTTP allows exactly this, a 404 from a server "not willing to disclose" that a resource exists ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.5)).

### 4.2

**Measured, not promised.** N3 copies the platform's latency targets, but on one host with no standby the demo cannot promise them, so it reports them with their sample sizes. A percentile is a statement about many samples: "p99 1 s" says one interaction in a hundred may take longer, so a p99 means little until there are hundreds of samples ([SRE book on objectives](https://sre.google/sre-book/service-level-objectives/)).

**Read the ranking with the faults paragraph.** N9 ranks first because it is the claim the recording is judged on (§18): two apps on one unchanged build. Each rank carries its reason in one clause, and N7's, "not traded", means no gain anywhere else justifies a leak. The faults paragraph makes the same split by kind of failure: a lost host is survived by snapshot and restore, while a cross-organization leak is "prevented, not tolerated", because nothing restores data once another company has read it.

### N2

**N2 counts restarts, not uptime.** N2, below, has no availability percentage to promise, since there is no SLA, so it is written from the recording's side: a restart may happen, and it must cost one redone step, never a lost act. It reports the maximum over twenty restarts rather than a percentile, because twenty samples are too few for a percentile and because one slow restart is one lost scene. Its last clause, zero platform-caused retakes in the last three rehearsals, reads as the readiness test.

### SR2

**Why both ends of every take.** The run at the start of a take replays against the restored state and the build about to be filmed, so a failure stops the take before the camera rolls; the run at the end replays the identifiers the take itself created, new work ids, a new record and idempotency keys, which did not exist at the start (11.1, steps 4 and 6). "Behind M29's allowlist" places the suite, like the whole host during rehearsals, behind a list of the only addresses allowed to connect: the recording machine and the operator's.

### SR4

**What anchoring the manifest digest buys.** The packet outlives the video: a partner may read it weeks later, and nothing in a folder of files stops someone editing it. So after each take the recorder computes a digest of the packet's manifest, the list of what the packet holds (a digest is a short hash that changes if any byte of its input changes), and writes it as an audit event. The audit trail is a hash chain whose latest link is copied to storage under Object Lock, which in compliance mode no user, the root user included, can overwrite or delete before its retention date ([S3 Object Lock](https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-lock.html)), so the recorded digest cannot be quietly changed later. Anyone can recompute it from the packet and compare: an edited packet no longer matches. The rest of SR4, a scan for headers, cookies, tokens and keys, keeps the packet from carrying a credential to every recipient.

### 5

**Read each act's "Captured" line first.** Each act gives its mechanism, its fixture values and what the recording keeps. The "Captured" list is the act's acceptance test and the mechanism is how the build produces it; a builder who starts from the captured evidence builds what proves the claim rather than what merely looks right. The six lists together are most of the packet (S07).

**The spike is the first thing you build.** "Prototyped" names what is already measured, the four-kind canvas, and says that nothing of the demo's own is. The spike (D8, R11) is the first real work: one complete path from definition to reopen, with the duplicate submission and the version conflict, on the real library and route. It runs before any scene because its numbers can change decisions (5.3).

### 5.1

**What the context picture argues.** A context diagram usually shows the users and the services a system calls; this one also draws an adversary as a user. The canary's arrow carries the proof on its label, 404 before any data and audited, and the canary enters through the same door as Maya (B10 is "a second B1 crossing"), so its replays test the real path rather than a special one. **Why the two outside systems see opposite halves.** fAuth knows who signs in and never sees data; the model provider sees schemas and synthetic rows and never learns who asked (N8), so a compromise of either yields only half the picture.

### 5.2

**Five database roles in one process, and why that is not one shared credential.** The four platform roles, `workspace`, `control`, `data` and `worker`, run as modules of one binary, but each opens its own connection pool as its own PostgreSQL role, a database identity with its own grants (permissions to insert into or update particular tables). So a bug in a page handler cannot insert a publication: PostgreSQL refuses the `workspace` role, whatever the code forgot. The trap this avoids is 12.2's option B, every module through one credential, where any bug can write anything. What the design does not buy is protection from hostile code: the process runs as one cloud identity, its task role, which may connect as all five roles, so an exploit inside the process can open any pool. The walls stop mistakes, not an attacker already inside (AR2). The fifth role, `reset`, exists only for the rehearsal reset (6.7).

**The burst one action causes.** The averages, 0.8 actions and 4.8 query operations a second, are not what sets latency here. One filter change fans out to six queries at once, one per linked view, and the data service runs only two of one organization's queries at a time and queues the rest, so a refresh arrives in waves. That is why "one actor at a time" is part of the design rather than a stage direction: the rehearsal script, not the code, keeps the burst at six (11.1, 11.2).

**A restart that is not a task replacement.** A load balancer probes its target every 10 s and takes it out of service after a run of failed checks, three here ([target group health checks](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/target-group-health-checks.html)). Without a supervisor, a crashed binary would end its container and ECS would replace the task: pull the image, start it, register it, pass a health check, a sequence nobody has timed against 30 s. The supervisor, a small process inside the container whose only job is to restart the binary, brings it back in place before the third failed check, so the restart costs only the process start, its pools, one key-set fetch and the browser's reload. One full task replacement is still timed, apart, because that number decides which host the demo runs on (13.2).

### 5.3

**Why only three doors are one-way.** The test is what reversing a decision would take. D2: rewriting every key in every store, because the organization id is in all of them from the first row. D3: re-proving every contract, because a fork would make act 6 prove a different implementation. D7: someone else's action, because once YC holds the packet nobody here can relabel it. Every other row is a reduction or a choice the platform's own mechanism can replace later. The one-way and two-way door language comes from [Amazon's 2015 shareholder letter](https://s2.q4cdn.com/299287126/files/doc_financials/annual/2015-Letter-to-Shareholders.PDF).

### D5

**Why "a kind is a release".** D5's row below fixes the component library. A component kind (a summary, a table, a timeline, a form) is renderer code compiled into the build, so adding one during the recording would be a deploy between acts, which N9 forbids; every kind both apps use is registered before the build digest, the hash of the exact image that is filmed, is taken. The row also names two operators, registered calculations an app definition may call: the CIDR lookup, which matches an address to an inventory network, and the integer currency operators, which keep money in whole cents. **What 426 B of headroom means.** The route has two budgets: Forma's own code, at most 28,100 B compressed, of which the fixture's four kinds used 27,674 B, and the route's 365 KiB of JavaScript, against which each kind's own code counts (6.5). Only the first is scarce, and a new kind spends it only by importing a Forma module the four did not, `createResource` or the HTML runtime; so what a kind costs is not its own size but what it pulls in from the framework. That is why the spike imports all nine before any scene (R11), and why the way out, FORMA-232's application budget or a kind cut, is a two-way door.

### D6

**Why a script and not administration pages.** Pages only the operator uses would be built and tested before any pilot asks for them (12.2's table), while a script that calls the platform's own administration API exercises exactly the interface a later page will call: the seam. The cost is "kept true by hand": nothing stops the script being wrong, so the pre-take assertion compares the live memberships and policies with the script's manifest before every take (6.7, 11.1).

### D8

**Why measure before rehearsing.** Each of the spike's numbers can change a decision: the route's bytes decide whether a kind is cut (D5), the commit's latency tests N3's commit target (p50 300 ms, p99 1 s), the restart maximum tests N2's 30 s. Found on camera, any of them would waste takes (12.2's table). The cost is that the first rehearsal waits. **Pitfall.** Treating the spike as a throwaway prototype. It runs the real path on the real library and route (R11), so what it measures is the code that will be filmed.

### D9

**A public client: why the row holds no secret.** In the authorization-code flow the browser comes back from fAuth with a one-time code, which the platform exchanges for a token ([RFC 6749, section 4.1](https://datatracker.ietf.org/doc/html/rfc6749#section-4.1)). A confidential client would prove itself at that exchange with a client secret; this one is public and uses PKCE instead: at the start of each sign-in the platform makes a random code verifier and sends fAuth only its hash, and at the exchange it sends the verifier, which fAuth hashes and compares ([RFC 7636](https://datatracker.ietf.org/doc/html/rfc7636)). A code stolen on its way back is useless without the verifier. The registration row then needs no secret, only the client's identity and its one redirect URI, the single address a code may be sent to, so there is nothing in it to store, rotate or leak into the packet. It is added by hand because fAuth at the pin has no API for clients (AR4).

### D10

**The one decision that reduces rather than binds.** The platform runs on AWS in one region (platform D15). The demo keeps the region and the core services, ECS, RDS, S3, KMS and a load balancer, but runs one task instead of four services spread over two zones, with no standby, no content network and no firewall (11.1). Notice the cost column, "money, metered": this is the only decision whose cost arrives every month whether or not anyone rehearses (11.2), which is why its cheaper alternative, a host that costs nothing, gets its own argument in 12.2.

### 6

**Every card must justify its box again.** In the platform each card is a separate deployment, a box because a network separates it. Here all four share one process, so each card answers "Why still a boundary" afresh, and the answer is always what one role's credential can do that the others' cannot: write a publication, read Parquet and commit a record, reach the provider. Read each card for that line and for "In a rehearsal", the demo's version of the platform cards' "What can go wrong": the failure that costs a take, not an incident. **Notice** that the cards name no extension point of their own, because the platform's hold unchanged (the section's first line). To practise Lesson 4's form, "to add Y you add Z without editing W", use 6.8's seams instead: 6.1's is already written that way, real membership replacing the script "without touching authority".

### 6.1

**Why a token without `tenant_id` mints nothing.** The claim is optional in fAuth's tokens, and the platform takes the organization from that claim alone, so a token without it has no organization to give a session. Refusing is what failing closed means here: there is no default organization a token could fall into.

**A restart is a reload, not a sign-in.** Sessions are rows in PostgreSQL (DS4), not memory in the process, so a restart signs nobody out. That is what makes N2's 30 s possible at all: otherwise every restart would send three actors back through fAuth's page.

### 6.2

**What the demo changes, and the failure only a script can cause.** The approval is the platform's, unchanged: Sarah approves a content digest, a hash of the exact admitted definition, so any change after her approval makes a new digest that the approval no longer matches. The card's example is that case, R4's test that "a later change invalidates the approval"; in the recording the denominator is patched in act 3, before anyone approves (7.1). What the demo changes is who sets policy: a script, through the platform's administration API (the administration group). A wrong value there crashes nothing; it changes what a take can show, which is why the pre-take assertion reads each organization's policy row against the manifest before every take.

### 6.3

**The server simulates nothing.** S05's lost response is made by the rehearsal script, which drops the response at the browser; the server receives a real second request and answers it with its real logic. With the fake on the client's side, the server's behavior on camera is the behavior that ships. **Why the reset deletes outcomes.** An outcome is the stored answer to a command's idempotency key, returned to any retry for 24 hours (section 9). If the reset left them, the next take's `ik-1` would come back with the previous take's answer, a dead take answering a live one (TH13).

### 6.4

**The heading's P numbers are the platform's.** Diagram elements keep the platform's ids (7), so P5 is the composer worker, P6 the import parser in its sandbox and P7 the index builder, which embeds each published app's description, examples and field names into one index file an organization, rebuilt on publication (platform 6.4, platform 7). The card names P7's work only as "indexing", and 7's "P2 to P5 and P7 share the one process" puts it in the binary with the rest. **Why act 6 navigates by alias.** P7's index, the one retrieval searches, can lag a publication by 60 s (platform A17), while an alias resolves to the app's stable id without the index (platform 6.1), so the scene cannot catch the index mid-rebuild.

### 6.5

**The filter epoch, concretely.** Every filter change increments a counter, the filter epoch, and every query carries it; a response that arrives with an older number is dropped. In the example, selecting the destination is epoch 12, so the timeline's slower answer for epoch 11 is thrown away instead of briefly showing a number from the previous filter, and six linked views never mix two filters on screen.

### 6.7

**Two filters on the evidence.** Chrome's sanitised export leaves out exactly three headers, `Cookie`, `Set-Cookie` and `Authorization` ([Chrome DevTools](https://developer.chrome.com/docs/devtools/network/reference#save-as-har)), and keeps everything else. So the recorder files only what it needs, status codes, request ids and the named `/v1/` bodies, drops every remaining header itself, and refuses a capture that still carries one of the three, the sign of an export made unsanitised. Two independent filters, Chrome's and the demo's, mean one mistake does not put a credential in front of every recipient (TH6).

**Two conditions, not one, for the reset.** The route registers at startup only when the rehearsal flag is set and role `reset` exists, and the role is created by the provisioning script, never by a migration. A deploy that copies the flag into the platform's task definition by mistake still finds no role there, so the route does not exist (TH1). A destructive operation behind one switch is one mistake away from a real organization; behind two switches set in different places it takes two.

**Read "a memento" literally.** A fixture state, both organizations' rows, objects and index file at the end of a clean act, is what [the memento pattern](https://refactoring.guru/design-patterns/memento) names: saved state that the reset restores without the rest of the code knowing how it is laid out. Carrying the build digest and the audit sequence inside it means a state from another build, or another time, cannot pass for a fresh one.

### 6.8

**Read the table as an agreement between two records.** Each row pairs a way the demo is smaller with the platform interface that must already exist in the demo even though the demo barely needs it: the organization id in every key with only two organizations, a versioned policy table with one script as its administrator, expected versions with nobody co-editing. A reduction without its seam would be a shortcut the platform pays for later, and the "Card" column says where in section 6 each seam is built. **Notice** that the line above the table repeats section 2's four non-reductions, so the list of what may not be faked sits where the reductions are. The seam column's D and C numbers are the platform's, as its heading says, and stay unlinked: its D3, for one, is the platform's pooled tenancy, the organization id in every key, which this record's D2 binds.

### 6.9

**Why act 4's restart drill runs with no job in flight.** A worker claims a job for 120 s, its lease, and no other worker may take the job until the lease runs out. A restart in the middle of a job therefore leaves it stuck for up to two minutes, four times N2's 30 s, so the drill is staged while no import or generation is running; otherwise the stuck job would be the step that is not redone in time.

**"Hard if changed".** Forma 3.0.0 and DuckDB are linked into the binary, so they cannot be down, but an upgrade of either is a dependency change; under the one-digest rule (N9) an upgrade between acts would also be a deploy.

### 7

**Two F numberings, and one new boundary.** This record numbers its own arrows, F1 to F13 on diagram 03 and F14 to F19 on diagram 04, and cites the platform's as "platform F22" and so on; 7.1 uses mostly the platform's, 7.2 and 7.3 the record's own. The diagrams keep the platform's element ids (P, DS, EE, B) with three changes, and the one to remember is B9, the operator's machine. The platform has no boundary there; the demo's operator runs two scripts on a laptop that also holds every key, every actor's session and the packet (AS13 in 10.2). A boundary is wherever trust changes, and the threat model gives this one its own row (TH15).

### 7.1

**Read acts 1 to 5 as citations.** These acts are the platform's flows 1 and 2 run on one process, so each phase cites the platform's arrows and adds what the demo keeps as evidence. The line to read in each act is "That is the end", which names exactly what the act leaves behind: one session and one audit row for act 1, two revisions for act 2, five revisions for act 3. **Notice** in act 3 that undo is revision 4 and reapplying it revision 5: undo never deletes, it adds a revision, which is why act 4 submits revision 5.

### 7.2

**What is the platform's and what the demo adds.** F7, the one transaction, is the platform's command unchanged (its F46): the outcome lookup, the version check, the epoch check and the writes inside one PostgreSQL transaction the data service owns. What the demo adds is the circumstances around it: the lost response is the script's, labeled simulated; the canary's replay rides the same arrows; the recorder reads only what the browser received. B4 drawn inside B2 is the one-process change: the data service's boundary is a database role inside the host's network instead of a network hop, and the check it makes is the same.

**Count what the conflicts add.** The happy path ends with two transactions, two record revisions and two audit events. Notice what the retry and the stale edit add to that count: no revision and no audit event, only an attempt logged apart and a 409. A test can assert those counts directly, which is how "one mutation" becomes something the packet proves.

**For Lesson 5's sequence diagram.** This record draws none of its own, since "platform 7.3 holds its sequence", with the P2 to P4 hop a call in process here. Use the worked example's steps 3 to 5 instead, whose F numbers walk the same messages on diagram 03, and ask at each arrow what a forged or lost message would do: step 4 is that question answered for one lost response, and step 8, the canary's replay, for a forged identifier.

### 7.3

**Why the operator's flows get their own diagram.** A threat row names the element or crossing it attacks, and the platform's diagrams draw no operator's machine as a boundary, no reset and no recorder, so five of this record's threats, TH1, TH2, TH5, TH8 and TH15, would have no arrow to name. Diagram 04 gives each of them one. Read its arrows into B9 with TH15 beside you: what comes back to the operator's machine is a filtered export and the operator views, all of it synthetic, so the one machine that holds every key holds as little else as the demo can arrange.

### 8

**Why the unique index, not the row lock, holds version 0.** A command locks the record row `FOR UPDATE` before checking its version, but before the first decision there is no row to lock, so two first decisions on EXP-002 could both see "absent" and both try to write version 1. The unique index on (organization, work, record, version) settles it: the second insert of version 1 fails, so one commits and the other gets a conflict ([unique indexes](https://www.postgresql.org/docs/current/indexes-unique.html), [row locks](https://www.postgresql.org/docs/current/explicit-locking.html)). Creation is the one case a lock cannot serialise, because there is nothing yet to hold.

**An orphan, never a dangling row.** Object storage acknowledges a write when the PUT returns, before the row that names the object commits. If the process dies between the two, an object exists that no row names, an orphan that wastes a few bytes; the reverse order could leave a row naming an object that was never written, a dangling reference that breaks every read of it. Choosing the order of two writes chooses which failure you get.

**Why retaking an act retakes the acts after it.** The recording joins one kept take per act, and each act's state is captured at the end of its kept take, so the next act starts from exactly what the camera saw: Sarah's approval in act 4 is of the digest act 3 formed on screen. Retake act 3 and its revision 5 may come out different, since the live model's arrangement need not repeat (13.3), so the `s3` captured from it differs too, and an act 4 kept from the old `s3` would approve a draft the recording no longer shows. The chain is the price of the stronger claim, every act continuing the one before, and it is what lets 13.3 answer "One take or many?" with one kept take per act.

**What recovery is not.** RPO, how much recent data a restore may lose, is one rehearsal, because the only snapshot is taken before each rehearsal; RTO, how long a restore takes, is unknown until S06's drill times one. "Reseeding fixtures is not recovery": loading the fixtures again proves nothing about restoring what a take committed.

### 9

**Retrieve before you re-submit.** After a timeout the browser cannot know whether its command committed. It asks first, by key, for the stored outcome, and re-submits under the same key only if there is none, so a command that did commit is not sent again; and if it is sent again anyway, as the rehearsal forces, the server returns the first outcome because the key is checked before anything else. Two layers: a careful client, and a server that does not depend on the client being careful.

**Why 409 and not 422 for a reused key.** The IETF draft on the `Idempotency-Key` header suggests 422 when a key comes back with a different payload ([the draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/)); the platform answers 409, Conflict, and the demo serves the platform's API unchanged. The argument (platform 9) is about what is wrong: 422 says this request's content cannot be processed, but the request is well formed and only conflicts with an earlier request stored under the same key, part of the resource's current state, which is the conflict 409 names in HTTP ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.10)). One code for both conflicts, the stale version and the reused key, also gives the browser one way to recover: the authorized recovery view.

**404 for an absent operation, 412 for a failed precondition.** The reset's guard uses two different refusals on purpose. Where the flag is not set the route does not exist, so it is 404, like any unknown path. Where it exists but an organization lacks the synthetic label, the build digest differs from the state's or the manifest digest does not recompute, the operation is real and its precondition failed: 412. The two codes tell the operator which of two very different things went wrong.

**Where the authentication column is.** This section's table lists scenes and error codes rather than entry points, because the API is the platform's, unchanged; the demo's entry points and how each one authenticates are in 10.2's interface table, which is where to apply the method note's test.

### 10

**When the data is fake, the asset is the claim.** Every row is synthetic, so a leak harms no person; the impact scale is about the recording's claim instead: a leak to the canary, a seeded step passing as working or a lost audit row ends a goal (G5, G6). That is why the controls still run as for Confidential data, and why 10.4 adds "passes a seeded step as working" to what High impact means. Most of the section cites the platform; read what is written in full, which is what one process and one operator change.

### 10.1

**SA3 names the only hostile principal.** In a rehearsal the one attacker who actually attacks is the canary, scripted by the operator; everyone else is trusted and audited. Real outsiders enter through the risk profile instead: a scan of a public host over weeks is rated moderately likely, which is why the load balancer admits only listed addresses during rehearsals (M29).

### 10.2

**The platform's families this section extends.** 10.2 adds to three lists it does not repeat, so here they are, from platform 10.2. The trust levels: TL0 anonymous, TL1 a member, TL2 a builder, TL3 a reviewer, TL4 an administrator, TL5 the operator just in time, TL6 an untrusted author (a dropped file until parsed, the model's proposal until admitted), TL7 the service identities, here one task role carrying five database roles, and TL8 the external systems and the release path. The invariants: SI1 scope, every read and write scoped by the verified session's organization, never a request value; SI2 authority, which no model output, file, link or stale job widens; SI3 governed writes, members seeing only publications `control` wrote over an approved digest and records changing only by declared commands; SI4 revocation within 5 s; SI5 an append-only audit; SI6 fair share between organizations. The assets: AS1 sessions and identity keys, AS2 publications and policy, AS3 records, work and the other Confidential rows, AS4 evidence, AS5 keys, AS6 index files, AS7 audit, AS8 the release path and logs. With them a row traces to the end, as Lesson 6 asks: TH4 puts SI1 at stake, and its "AS3, AS4" are the records and the evidence.

**Why the whole binary is in the trusted computing base.** The trusted computing base is the set of components that must work correctly for the invariants to hold ([Building Secure and Reliable Systems, chapter 6](https://google.github.io/building-secure-and-reliable-systems/raw/ch06.html)). In the platform it is a list of enforcing components spread over separate deployments; here, because one task role can connect as every database role, a flaw anywhere in the binary can reach everything, so the base is the whole binary plus the reset's guard and grants and the recorder's filter. The record says so in one sentence rather than claiming a small base it does not have.

**Two invariants about honesty.** SI7 and SI8 protect no data; they protect the demo's truthfulness. The reset restores only a digest-matched state and never deletes audit or sessions, and the packet holds only what the browser received and the operator views returned. A threat model can carry invariants for the claim as well as for the data, and enforce them with the same tools: grants and a filter.

### 10.3

**The platform's actors, cited bare.** 10.3's opening line keeps the platform's TA1 to TA9 and drops TA10, so here they are, from platform 10.3: TA1 an outside attacker, TA2 a member of another organization, the canary's role here, TA3 a malicious or phished member, TA4 a builder steering the composer, TA5 an instruction injected into evidence, TA6 the model provider, TA7 a malicious dependency, TA8 a compromised CI, TA9 the insider operator, and TA10 future management, an acquirer or a court. The threat rows cite them the same way, so "(TA9, TA11)" in TH1 is the platform's insider operator beside this record's script bug.

**The most likely attacker is your own script.** TA11, a bug in the operator's scripts, is "the commonest cause": a wrong grant, a wrong policy kind, a wrong redirect URI, or the rehearsal script faulting the wrong step. Listing it as a threat actor puts the most probable failure through the same analysis as an attacker, so it gets a control, the pre-take assertion, rather than a hope. The platform's TA10 goes the other way, and is removed: future owners or a court have nothing to compel when every row is synthetic.

### 10.4

**The branch the platform lacks.** Branch 4 of the attack tree, through the operator, is new: the shell's keys, the operator's machine, the packet and the recording, and a session surviving the reset are all surfaces the demo adds to the platform's. Read the four branches as a list of walls: the first three meet mostly the platform's controls (branch 3's fixture state, TH2, is the demo's own), the fourth meets controls this record writes in full (TH5, TH6, TH9, TH15).

**Why the canary's row is the only High and High.** TH4 is rated High on likelihood on purpose: the canary suite attempts that attack on every take, so it is certain to be tried. A scripted adversary turns likelihood into something you arrange, and the controls are measured against it every time instead of on the day a real one arrives.

**Pitfall.** The paragraph after the table re-rates three of the platform's rows, platform TH8, TH11 and TH14: the forwarded principal, the owner credential and the flood. This record's TH8, TH11 and TH14 are three different threats; the prefix is the only thing that tells them apart.

### TH1

**The demo's signature risk.** In TH1, the first row below, the reset is a destructive operation that ships inside the platform's own binary, and the threat is a deploy mistake that switches it on where real organizations live. The response has the two halves a High-impact row needs: a preventative control, two conditions the platform's deployment never meets, and a detective one, a test on every build that starts the image as the platform would and asserts that the route is 404 and the role absent. A control tested on every build is evidence; a control that is only designed is a hope.

### TH2

**Keep the check away from the thing it checks.** A fixture state lives in object storage under the operator prefix, so whoever can write there could alter it; if its digest lived beside it, they could alter both. So the digest lives in the pin file in the repository, at the build's tag, and the reset recomputes it before restoring: a tampered state is refused with 412. The detective half, the recorder diffing each restored manifest against its capture, catches what gets past the first.

### 10.5

**Why the service token is not minted.** In the platform a call between roles crosses a network, so it carries a signed, short-lived token naming caller, callee and principal. Inside one process there is no network to cross, and a signing key would add a secret to protect without adding a wall that code in the process could not climb. A typed principal, a value only the authorizer's module can construct, gives the same protection against honest mistakes for nothing: a handler cannot fabricate one. When the roles split into services (G7), the signed token returns; until then it is "a seam, not a control".

**Who reads the audit when there is no second person.** With one founder there is no two-person rule and no breakglass reviewer (AR1). The record's answer is that the audience reads the audit, through the packet's anchored manifest (M31): the reader moves outside the company, the demo-scale substitute for a second operator.

**Notice.** "(F7; platform SA2)" in the record's second paragraph cites the platform's assumption that no internal caller is trusted, which is why the callees re-read membership (platform 10.1); this record's SA2 is a different assumption, fAuth's client row.

### 10.6

**One key per retention need.** Each organization's data sits under its own key, and the operator prefix, which holds the fixture states and the packet, under the platform's own (section 8). After delivery both organizations' keys are scheduled for deletion, which makes everything encrypted under them unreadable at once (crypto-shredding), while the packet stays readable because it was never under those keys. KMS does not delete a key immediately: it waits at least 7 days, during which the key cannot be used and the deletion can still be cancelled ([KMS key deletion](https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html)); the platform's 24-hour hold comes before the schedule.

**Read the fail-closed list as a choice of which failure to pay for.** Every control in it can fail in one of two directions, and the record picks the one that costs a take: a failing canary suite could let the take run, or stop it; P10 could file a capture it cannot filter, or refuse it. Failing closed spends a retake, which a rehearsal can afford, to avoid what no retake undoes: a leak to the canary, a credential in the packet, a seeded step accepted as working once YC holds the packet (D7). A control that fails open turns its own outage into the breach it exists to prevent.

### 10.7

**What "accepted" means.** An accepted risk is a decision, not an omission: it has an owner, the head, a reason, and usually a bound, such as AR5's route proven dead on every build or AR1's every operator action listed against the script. Writing it down lets a reviewer disagree with a specific sentence, and lets the risk be revisited when its reason changes, for example when a second person joins. **Pitfall.** Reading "the platform's AR2 to AR11 hold" as these rows. The numbers collide again: this record's AR2 is one host and one process, the platform's AR2 is its pooled roles. The paragraph under the table does the same with SR: "platform SR1, SR2, SR4 to SR7 and SR10 to SR13 hold" names the platform's requirements, not this record's SR1 to SR6.

### AR2

**The grants bound bugs; the suite only detects the rest.** AR2's reason, in the second row below, splits the defence in two. The database grants stop an honest mistake from writing what its role may not write; the canary suite, run every take, detects what grants cannot see, such as a handler that forgets to scope a query to the session's organization. What neither stops, hostile code running inside the process, is the risk being accepted, with one rehearsal of data, the RPO, as the most that losing the one host can cost.

### 11

**Operations for takes, not for a service.** Nobody runs this as a service and nothing promises uptime, so the unit of operations is the take, and the question each part of the section answers is what a failure costs a recording: a redone step, a retake or the rehearsal. Read it as the procedure that keeps a recording honest and repeatable.

### 11.1

**What the reset cannot restore.** A fixture state holds only what the platform stores, rows, objects and the index file, so a reset cannot reach the browser or fAuth, and it leaves the platform's sessions alone on purpose (TH9). The one starting point act 1 needs, Maya signed out of the platform and of fAuth, whose live session would skip its page, is therefore something no reset can produce: step 3 of the take sets it by hand, and TH9's control counts on it (M17).

**One digest, no waves.** The platform rolls a release out in waves, so that a bad build reaches only part of its traffic first; the demo has one build for the whole recording, so there is nothing to stage. A new image means a new rehearsal and newly captured states, because each state carries the build digest it was captured on.

### 11.2

**The driver, and what "$17 a rehearsal" means.** The largest line is Fargate, $85 of about $170, and its driver is the task's size, 2 vCPU and 8 GiB, not how much the task is used. "$17 a rehearsal" divides the month by ten rehearsals, so it is mostly fixed hosting spread over them; one more take costs only its tokens, as the paragraph under the table says.

**Why N2 and M19 depend on the 8 GiB.** A parser bomb that passes the byte cap meets A6's 50 MiB expanded cap next, which stops the parse and refuses the file (7.1); the module's 2 GiB memory cap is the backstop behind it, the point at which the runtime stops the module whatever the parse is doing. That backstop protects the process only if the module's 2 GiB fits beside everything else the process holds: the binary, two DuckDB operations, the result cache and some headroom. Sized to that sum, even a parse that got past the expanded cap would cost only the module; on a smaller task it could take memory the process needs, and the take would spend a restart, N2's 30 s, on what should have been a captured refusal.

**The spend limit is a safety stop.** The provider key sits in its own account unit with a spend limit just above the month's tokens, and hitting the limit is an error that is never retried: a runaway loop ends a take instead of running up a bill.

### 11.3

**Alerts sized to the scene.** The platform raises a ticket for a job queued over ten minutes (platform AL6), right for a service whose users can wait and wrong for a five-minute act; the demo pages on `provider_unavailable` within 1 s and ends the take. An alert's threshold follows the time scale of what it protects. The other page, any canary failure, is the platform's own: it means isolation may have broken, which is never something to leave until morning.

### 11.4

**Read the last column.** Where the platform's table ends in diagnostics, this one ends in what happens to the take: it continues, waits, restarts the act or restarts the rehearsal. The decision is made in advance and in writing, so nobody improvises during a recording. The runbook rule under the table makes the procedure learn from the rehearsals, and its trigger is the second time: a one-off accident stays one blameless log line, and only what recurs earns a script step or a pre-take check that runs every take from then on.

### 12.1

**Drawbacks priced in takes and seconds.** Each drawback names what it costs the recording and what catches it: one host makes a restart a scene, timed against N2; manual provisioning can over-grant an account, found by the assertion and the canary suite, and "nothing prevents it"; the demo-only route is dead without flag and role, proven every build. The phrase to copy is "nothing prevents it": a design that says which of its failures are only detected, not prevented, tells the reviewer where to look.

### 12.2

**"Reuse or buy", ranked by reuse risk.** The parenthesis after each option says how much control the reuse would take from you: a library you call (a deck of mock-ups), a pattern you copy (design B v5 rebuilt), a framework that calls you and owns the control flow (a no-code tool). The more control it takes, the more of the claim rests on someone else's code; the no-code tool loses outright because it runs generated code as trusted, the thing the platform's typed definition exists to avoid.

### D1

**Option B is the trap this design is one setting away from.** One process with one database credential is the simplest option, and it lets the code that handles model output insert a publication. The chosen design differs from B in one respect, each role connecting as its own database role with its own grants. The database itself is still shared, on purpose, so that a record, its outcome and its audit row commit in one transaction; what is not shared is the credential. ([The shared database pattern](https://microservices.io/patterns/data/shared-database.html) lists the coupling costs of sharing a database; the record's objection to B is a different cost, authority.) The confirmation is a test that can fail, run on every build: `workspace` and `worker` try to write a publication through their own pools.

### D2

**Why the keys are right from the first row.** Of the rejected options, one organization per rehearsal leaves the canary nothing to replay, and adding the id later turns the seam into a migration of every stored row. Only the chosen option lets the canary suite test the platform's real keys on camera; isolation is the recording's claim, and keys cannot be retrofitted between takes.

### D3

**"No fork" is what makes N9 structural.** If the demo had its own definition format or its own admission, act 6 would prove the demo's code rather than the product's, and every contract would have to be proved twice. Using the platform's `AppDefinition`, admission, publication and command code means that N9's zero deploys between the two apps follow from the design instead of from discipline. **Pitfall.** Simplifying admission because it is only a demo: that is the fork this decision refuses, and full admission at demo scale is the price the record accepts for refusing it.

### D4

**What "two-way at the protocol" means.** The adapter fixes the contract, a verified token carrying `tenant_id` and `sub`, and leaves the provider behind it free to change: fAuth now, the platform's minimal login as the fallback. Choosing fAuth also means that the organization the canary is refused by, and the person a revocation acts on, come from a real tenant boundary in a signed token that the demo did not have to build, rather than from a password store made for the recording. The dropdown, option C, loses not because it is short but because it is not a reduction: it removes authentication instead of shrinking it.

### D7

**The door that closes on someone else's desk.** D7 is a working rule until submission, whose cost is retakes, and one-way after it, because the packet is then in YC's hands and a step cannot be relabeled. Most one-way doors are expensive because of stored data; this one is expensive because reversing it would need the recipient's action.

### 12.3

**Abandoned here, not forever.** The MCP scene depended on a desktop assistant's handling of a local connector, outside the take's control, and the public page was reachable only on the machine that served it, so a partner could not test it; both, with the three surfaces, return as platform work (platform C14). Recording each idea's pitfall is what stops the next reader proposing it again without an answer.

### 13.1

**A risk scale measured in takes.** High ends a goal or the claim, Medium costs a take, Low moves a number: the same High as the threat table, with the lower grades translated into what a rehearsal loses. **Why RK3 is looked at first, though Low on likelihood.** A seeded step passing as working is unlikely, with the labels, the receipt check and a second viewer against it, but it cannot be undone once the packet is submitted (D7), and it voids the whole claim rather than one scene. Rank by what a failure would destroy, not only by how often it happens.

### 13.2

**Every question carries its current answer.** Each open question names the choice the build proceeds with until it is answered, so nothing waits on it; a question without one either stops the build or is settled in passing by whoever meets it first, with no record. Notice also that the "Still unknown" line holds only what a measurement will settle, the spike's numbers, the timed task replacement, the RTO from S06's drill and a month's cost from the meter, never a decision someone has yet to make.

### 13.3

**Answered by evidence, not assurance.** Each objection is answered with a pointer to sections, goals and fixtures rather than with a new argument, which is the test that the body already answers it. Notice the first, "Made on camera, or loaded?": its answer is one build digest and zero deploys, things the packet shows, rather than a promise. And notice the form of the third, "What is simulated?": an inventory of every labeled step under its label, which a skeptic can check against the packet item by item, where a general "nothing important is simulated" could only be believed.

### A

**This edition's glossary has two sources.** Appendix A repeats the record's own inline definitions, and its first sentence names the shared platform terms, from admission to space, that the platform's glossary defines. This layer defines those shared terms again for this edition, so you can read the demo without the platform's glossary open; both sets are merged on the Glossary tab.

### C

**The numbers move between records.** Appendix C's mapping sentence is a translation table: the platform's TH1, a member of another organization reading Northstar's data, is this record's TH4, and the platform's TH4, the parser bomb, is this record's TH10. Read "platform TH" with its prefix and use the mapping, or you will compare two different threats. **Notice** the three rows that are new, TH8 (the reset racing a command), TH14 (a deploy between acts) and TH15 (the operator's machine): each is a threat the platform cannot have, because it has no reset, no recording and no scripts run from a laptop.

### TH9

**Why sessions surviving the reset is a threat row.** Keeping sessions makes retakes quick, and it means a cookie from the previous take still works in the next. The control is the epoch: every request checks it with nothing cached in front of it, and the reset bumps it, so every tab re-authorizes within 5 s against the restored rows.

### TH10

**One process changes the parser's worst case.** In the platform an escaped parser reaches what the worker role can; here the module runs inside the one process, whose task role reaches every pool, both data keys and the provider key, so escape is rated High on impact. Exhaustion is the likelier half, High on likelihood because any file a builder drops can attempt it, and the design meets it in layers: the byte caps refuse an oversize file before parsing; A6's expanded cap stops a parse that inflates past 50 MiB; and behind both, the module's memory cap and its fuel, the runtime's budget of work, enforced below the task's memory, are the backstop, so that whatever gets past the first two kills only the module, never the process (M19; 7.1, 11.2).

### TH14

**A threat the video cannot show.** A deploy between acts 3 and 6 would change nothing on screen, because Spend Review looks the same on a different build, and its actor is the operator (TA9), so the operator's own word that nothing was deployed proves nothing. The control therefore reads what the operator does not write by hand, the task's image digest and the cloud's deploy log, and the row is High on impact at Low likelihood because what it would void is N9, the claim 4.2 ranks first.

### TH15

**The laptop as one point of total compromise.** Whoever controls the operator's machine gets every key, every actor's session, the unfiltered export and the pin file at once: everything the walls elsewhere protect. Nothing on one laptop can be walled off from whoever controls it, so M30 works on the two things a single machine still allows, how much is on it and for how long, and M31 adds a record made elsewhere, under the Object Lock anchor, so that tampering shows afterwards. What remains is accepted as the one machine (AR3).

## Glossary

- **take** — one recording of an act or of the whole run; the recording keeps one take of each act, each starting from the state the last ended in, and the per-take counts of 4.2, 11.2 and 11.3, N2's restarts among them, are of whole-run takes, up to twenty over ten rehearsals (appendix A, 4.2, 8)
- **rehearsal** — a timed whole run of the six acts, about 30 minutes, filmed at most twice, so one or two whole-run takes; ten are planned (appendix A, 4.2)
- **retake** — running a take again from the fixture state before its act, after a failure ended it; retaking an act retakes the acts after it, since each starts from the state the last ended in (8, 11.1, 13.1)
- **scene** — one of the six acts as the controlling definition calls them (§16; numbered 1 to 6 in section 9), or a filmed moment within an act that shows one mechanism, such as Maya's refused publish in act 4 (G6, D7)
- **actor** — one of the fictional people as played on camera, Maya, Sarah or Jordan, each in their own Chrome profile; not a threat actor (A13, 11.1)
- **Maya | Sarah | Jordan** — Northstar's three fictional people: Maya builds both apps, Sarah reviews and approves, Jordan reopens shared work and starts his own (A2, 4.1)
- **Northstar** — the fictional company whose Security and Finance spaces the demo runs in (2, 5)
- **Spend Review** — the demo's second app: a queue, a detail view and a decision form over a spending table in Finance (1, 7.2)
- **canary | canary organization** — the second synthetic organization, whose member replays Northstar's calls with Northstar's identifiers and must be denied every time (reading note, 5.1, R8)
- **synthetic data | synthetic rows** — made-up data that describes no real person or company; every number in the demo is computed from it (A10, 4.2)
- **fixture | fixtures** — fixed synthetic inputs with known expected results, the scenarios S01 to S07 (2, 4.2)
- **the head** — the person who commissions this record, approves it and owns every residual risk (cover, 10.4, 10.7)
- **the editions** — the versions of this record the factory derives for particular readers, such as a reader's edition with presenter notes and this annotated edition; A1's "If wrong: the editions" means a different audience changes those, not the design (A1; Start here)
- **L2** — the factory's middle document level, for one service with its own stores, API and operations built by one team: it sets the record's length and asks for every section's full content, a fresh reviewer and a security reviewer; the head set it at kickoff (cover; the reading note's K2)
- **phase 7** — the factory's finish phase, the seventh after intake in the Start here tab's account of how the record was made: the summary written last, the record built and sent through the gate, a script, fresh reviewers and the cross-checks; the platform record is cited at its phase 7 commit, the version that passed its gate (cover, 2, appendix B)
- **YC | YC partner** — Y Combinator, the startup programme the recording is made for; a partner there is the viewer the demo is judged by (A1, 4)
- **pilot** — a first real customer's use of the platform on real data, under agreed terms; out of scope here (3.2, 13.2)
- **controlling definition | §16 | §18 | §19** — the App Platform definition the product is judged against, restated for the demo in `PRODUCT-BRIEF.md`: §16 is the six acts, §18 the completeness test, §19 the section the non-goals cite (reading note, 2)
- **completeness test | completeness clauses** — §18's checks that the product is a platform: two apps through one creation, publication, access and saved-work model, a second user working independently, saved work reopening with its context, revocation holding, a published app surviving a failing draft (2, G1)
- **K1 to K5** — the head's kickoff rulings of 2026-10-22, recorded in both tasks' intake: K1 the typed definition, K2 the mode, K3 the stack tickets assumed done, K4 fAuth first, K5 Parquet and DuckDB (reading note)
- **C01 to C16** — the shared contracts between the platform and the demo, at lock edition `prep-2026-10-22.1`; this record cites them and restates what it relies on (reading note, 2)
- **FORMA-247 | FORMA-248 | FORMA-232 | FORMA-246** — tickets on the Forma work board: FORMA-247 fAuth as the demo's login, FORMA-248 fAuth's signing key in KMS, FORMA-232 and FORMA-246 assumed done under K3 (appendix B, 11.1)
- **BUILD-610 | BUILD-819** — the framework tickets behind the renderer's refusal of a computed kind and behind the 28,100 B route budget (TH12, reading note)
- **design A | design B v5** — the earlier records: design A, the security-first platform on markup modules that the platform record revises, and design B v5, its insurance demo, which this record revises in direction and carries nothing from (cover, 12.3)
- **markup modules** — apps written as generated markup rather than as a typed definition; the earlier designs' approach, reversed by the platform (12.3)
- **MCP** — the Model Context Protocol, by which an assistant application connects to outside tools and data ([MCP](https://modelcontextprotocol.io/docs/2026-07-28/getting-started/intro)); design B v5 had a scene on it (3.2, 12.3)
- **Forma 3.0.0** — the web framework the platform is built on, pinned at `2006423b`; its compiler limits and its route budget bound the browser workspace (2, 6.5)
- **28,100 B | route budget** — the most framework code one page, a route, may load, measured gzip-compressed ([gzip](https://datatracker.ietf.org/doc/html/rfc1952)); the composition fixture used 27,674 B, leaving 426 B (reading note, D5, 6.5)
- **HTML runtime** — the framework's code for mounting markup in the browser; the browser workspace stops fitting its route at the first component that imports it (6.5)
- **`createResource`** — Forma's primitive for data that loads asynchronously, re-running a fetch whenever its source signal changes ([Forma's core API](https://github.com/getforma-dev/forma/blob/v3.0.0/packages/core/docs/API.md)); none of the four measured kinds imports it, so a kind that does would be the one to spend the 426 B (6.5)
- **application budget** — the size budget an application's routes get under FORMA-232, the Forma ticket for route size budgets that K3 assumes done; 6.5 names it as the way past the 28,100 B ceiling other than cutting a kind (6.5; platform appendix B)
- **primitive | primitives** — a building block of the renderer library, a component kind or an operator; Spend Review uses some that Northstar's app never did (4.2, 6.5)
- **reduction** — a way the demo is smaller than the platform (fewer, smaller, by hand) that keeps the platform's interface for the thing reduced (6.8, R10)
- **seam** — the interface a reduction keeps, through which the platform's full mechanism later replaces it without changing what is built on it (6.8, R10)
- **status tag | [built] | [proposed]** — the bracketed mark saying how real a part is: [built] exists at a named pin, [proposed] is this design, still to be written, and [reduced: the platform's X] marks where the demo has something smaller than the platform's X, with a seam for X to return through; a tag may name the part it covers, as in [built: the rendering]; the platform's fourth, [deferred], designed for later, marks nothing in this record (reading note, 5; platform 5)
- **one-way door | two-way door** — a decision expensive to reverse, against one that can be changed later; 5.3 labels each ([Amazon's 2015 letter](https://s2.q4cdn.com/299287126/files/doc_financials/annual/2015-Letter-to-Shareholders.PDF)) (1, 5.3)
- **fit check** — the part of 5.2 that places the requirements' numbers on the components that carry them (5.2)
- **envelope** — the assumed scale the design is sized for, an upper bound rather than a forecast (3.3, A4)
- **fAuth** — Forma's own identity service, which the platform signs in through, run as its own deployment and pinned at `b750ca53`; it knows who signs in and never sees data (2, 5.1, 6.1)
- **tenant | tenants** — in fAuth, a separate group of users with its own sign-in settings; the demo has one per organization, `northstar` and the canary's (6.7)
- **`tenant_id` | `sub`** — the claims in fAuth's token that name the tenant and the person; a session is minted from these two alone (6.1, 10.5)
- **session adapter | identity adapter** — the one component that turns a verified sign-in into a platform session, so the identity provider behind it can change without the rest noticing (6.1, D4)
- **minimal login** — the platform's own simple sign-in, kept behind the same adapter as the fallback if fAuth fails; proven on every build, switched before a take, never during (A12, D4, 11.1)
- **authorization-code flow | code flow** — the sign-in in which the browser returns from the identity service with a one-time code that the client exchanges for a token ([RFC 6749, section 4.1](https://datatracker.ietf.org/doc/html/rfc6749#section-4.1)) (A13, 10.5)
- **PKCE | code verifier | S256** — proof key for code exchange: the client sends the hash (S256, SHA-256) of a random code verifier when sign-in starts and the verifier itself when it redeems the code, so a code stolen on its way back cannot be redeemed ([RFC 7636](https://datatracker.ietf.org/doc/html/rfc7636)) (D9, 6.7, 10.5)
- **public client | registered client** — an OAuth client the identity service knows by a registration but which holds no secret; PKCE ties each code to the client that started the sign-in ([RFC 6749, section 2.1](https://datatracker.ietf.org/doc/html/rfc6749#section-2.1)) (6.7, D9)
- **redirect URI** — the one address the identity service may send the browser back to with a code; registering exactly one means a code cannot be delivered elsewhere (SR3, TH3)
- **`state`** — a random value the client sends when sign-in starts and checks at the callback, so the callback is tied to the browser that began it (TH3, 10.2)
- **EdDSA** — the signature scheme fAuth signs its tokens with; anyone holding the public key can verify a token, so the platform needs no secret of fAuth's ([RFC 8032](https://datatracker.ietf.org/doc/html/rfc8032)) (5.1)
- **key set | `kid`** — fAuth's published set of public signing keys, each named by a key id (`kid`) that a token's header carries, so a token is checked against the right key ([RFC 7517, section 5](https://datatracker.ietf.org/doc/html/rfc7517#section-5)) (5.1, 10.5)
- **verified offline** — checking a token's signature against the cached key set without calling fAuth, so fAuth being down stops new sign-ins only (5.1, 6.9)
- **passkey** — a sign-in credential kept on the person's device instead of a password, built on the W3C's Web Authentication standard ([WebAuthn](https://www.w3.org/TR/webauthn-3/)); fAuth's server supports it and the demo leaves its button unwired (3.2)
- **`__Host-` cookie** — the platform's session cookie: opaque, carrying only a random id, and with a prefix that makes the browser accept it only over HTTPS from the host that set it ([Set-Cookie](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie)) (10.6)
- **sign-out everywhere | sign out everywhere** — ending every session of the actors at once; here it also bumps the membership epochs, so every open tab is denied within 5 s (10.6, TH15)
- **just in time | just-in-time** — granting a role only for the task at hand and letting it lapse after, so nobody holds standing power; the operator role lasts one take (TH5, 10.5)
- **operator role** — the platform role that reaches only the reset, the administration calls and the operator views, never an organization's data; taken just in time for one take (9, 10.5)
- **operator | operators** — the one person who provisions, resets, restores and files the packet, reading only synthetic data (4.1); in D5's "integer currency operators", instead, a registered calculation an app definition may call (D5)
- **breakglass** — an emergency path to powers beyond a person's normal role, logged and reviewed by someone else; the demo has none (AR1, 10.5)
- **two-person rule** — a rule that a dangerous action needs a second person's approval; impossible with one founder, so accepted away (AR1)
- **principal | typed principal** — the verified identity a request acts as; inside the one process it travels as a typed value that only the authorizer's module can construct (10.5)
- **service token** — the platform's signed, short-lived token between roles that run as separate services; not minted in the one process, and the seam for when the roles split (10.5)
- **authorizer | authorization point** — the one function that decides whether a request may proceed, re-reading the membership epoch on every request; the workspace server holds it (5.2, 10.5)
- **module privacy** — a language rule that only code inside one module can construct or call something; it stops an honest mistake elsewhere, not hostile code running in the process (10.5)
- **logic bug** — a mistake in honest code, such as a handler that forgets a check; the database roles' grants contain it, but nothing inside the host contains code an attacker runs in the process (5.2, SA1)
- **compromise domain | failure domain** — everything one break-in, or one fault, takes down together; one host is one of each (SA1, D1)
- **organization** — a company on the platform, with its own members, spaces, policy, data key and rows; Northstar and the canary are the two (2, D2)
- **space** — an area of apps inside an organization, Security and Finance here; grants, not the organization boundary, keep a Security-only member out of Finance (R7, 7.2)
- **alias** — a short name such as `/security` that resolves to the same stable id as a click (R1, 6.4)
- **app definition | `AppDefinition` | typed app definition** — the typed description of an app, its views, queries, records and commands, that the model proposes and the platform validates and renders: data, never code (K1, 5, 7.1)
- **component kind** — one of the renderer library's view types, such as a summary, a table, a timeline or a form; the demo ships the platform's nine and adds none (D5, 6.5)
- **CIDR | CIDR lookup** — the notation for an address range, an address and a prefix length such as /24 for 256 addresses ([RFC 4632](https://datatracker.ietf.org/doc/html/rfc4632)); the lookup matches a destination to the inventory, exact entry first (D5, 7.1)
- **integer currency** — money kept as a whole number of the smallest unit, `amount_cents` here, never as floating-point dollars, with one declared currency; a sum across currencies is refused (D5, N6, 7.2)
- **canvas** — the surface on which an app's views are arranged, rendered by one browser island (reading note, 6.5)
- **island** — a piece of interactive browser code attached to a page the server already rendered; the canvas is one (6.5)
- **typed props** — the plain values, flags, lists and numbers, that the server computes for a renderer to draw (5.2)
- **linked views** — views that share one filter, so a selection in one re-queries them all (R3, 6.5)
- **composer** — the assistant that turns an intent into a proposed definition or patch, through a job the worker runs (5, 6.4)
- **retrieval** — the search over an organization's index, the platform's catalog metadata and curated examples and later its published apps, that finds parts a new app can reuse; its index may lag a publication by 60 s (6.4, 7.1)
- **retrieved | adapted | generated** — the three labels on each part of a new app: found as it was, found and changed, or newly proposed by the model (R3, 7.1)
- **patch | semantic diff** — a change to a draft expressed against a named earlier revision; the semantic diff shows what changed in meaning, a measure or a field, rather than which characters moved (R3, 7.1)
- **preview store** — where trial form entries in a draft's preview are written, apart from live work, so publication carries none of them (7.1)
- **live work** — saved work on a published app, as opposed to trial entries in a draft's preview (R5, 7.1)
- **control plane** — the role that admits definitions and alone writes publications and policy (6.2)
- **admission | admission report** — the control plane's check of a proposed definition against the schema, the grammar, the complexity limits and the registered kinds and operators; the report says what it checked and refused, and a fixture proposal's report carries a `fixture` label (6.2, TH12)
- **publication** — the record, written only by the control plane, that an exact definition digest was approved under the organization's policy and now runs (6.2, 7.1)
- **independent review | owner publication | policy kind** — the two kinds of publication policy: under independent review an author's own approval is refused and someone else approves; under owner publication the owner may publish their own app (R4, 6.2)
- **data service** — the role that runs typed queries and commits record commands, the only one that reads Parquet or writes records (6.3)
- **receipt** — the server's record of exactly which sources, revisions, parameters and policy produced a number, so the number can be reproduced and checked (reading note, 6.3)
- **case | saved work | work id** — a piece of saved work: one id pinned to the publication it was started on, with its bindings, records and notes (reading note, R5, R6)
- **binding | bindings** — the link from a case to the source revision or record collection it uses; Jordan's new work has its own (R6, 7.1)
- **pin | pinned** — fixed to one exact version: a dependency at one commit, such as Forma at `2006423b`, or a case to the publication it was started on until an explicit adoption (2, R6)
- **adoption** — explicitly moving a case to a newer publication of its app, by compare-and-set; until then it stays pinned (R6, 7.1)
- **expected version** — the record version an edit was based on; if the record has moved on, the edit is refused with 409 instead of silently overwriting (reading note, 7.2, 9)
- **record command** — a declared command that changes an internal record in one transaction, with an expected version and an idempotency key; Spend Review's decision is one (R7, 6.3)
- **idempotency key | `Idempotency-Key`** — a client-chosen id sent with a command so that a retry is recognised as the same command; `ik-1` and `ik-2` in act 6 ([IETF draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/)) (9, 7.2)
- **outcome | stored outcome** — the stored result of a record command under its idempotency key, returned unchanged to any retry for 24 hours (6.3, 9)
- **recovery view** — the authorized current state returned with a 409, so the person can redo the edit on top of it (6.3, 9)
- **epoch | membership epoch | filter epoch** — a counter that a change advances so that a late answer can be told from a current one: the membership epoch moves on revocation or a reset and is checked on every request; the filter epoch moves with each canvas filter change, and the browser drops answers carrying an older one (reading note, 6.1, 6.5)
- **revocation** — removing a grant or a membership; the epoch moves, so the next request within 5 s is denied, queued results included; a download already made cannot be recalled (R6, 7.1)
- **work access | app access** — two separate grants, one to open a given case and one to run the app; a bare link carries neither (R6, 7.1)
- **export** — a labeled snapshot of selected findings with their source and app revisions and capture time; once downloaded it cannot be recalled (7.1, S04); in "the raw export" and "an unsanitised export", instead, the browser's HAR capture before the recorder filters it (6.7, 10.6)
- **row-level security** — PostgreSQL policies that filter every row a role reads or writes, here by organization ([row security policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)) (3.1, 6.6, 8)
- **`FORCE ROW LEVEL SECURITY`** — the table setting that subjects even the table's owner to its row policies; without it the owner is exempt ([row security policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)) (8)
- **`NOLOGIN`** — a role attribute that forbids connecting; a `NOLOGIN` owner of the tables means no connection ever acts as their owner ([CREATE ROLE](https://www.postgresql.org/docs/current/sql-createrole.html)) (8)
- **`BYPASSRLS`** — a role attribute that skips every row policy; no runtime role carries it, and neither does `reset` ([CREATE ROLE](https://www.postgresql.org/docs/current/sql-createrole.html)) (6.7, 8)
- **database role** — a PostgreSQL identity with its own grants; here one for each platform role, `workspace`, `control`, `data` and `worker`, plus `reset` under the rehearsal flag (5.2, D1)
- **grant | grants** — a permission a database role holds, such as inserting into one table; disjoint write grants mean no two roles share a write permission (5.2, 10.5)
- **connection pool | pool** — a set of open database connections a role reuses; here one per database role, each opened at start with its own 15-minute token (5.2, 10.5)
- **unique index** — an index that refuses a second row with the same key; on (organization, work, record, version) it turns two racing creates into one commit and one conflict ([unique indexes](https://www.postgresql.org/docs/current/indexes-unique.html)) (7.2, 8)
- **`FOR UPDATE` | `FOR SHARE`** — row locks taken inside a transaction: an exclusive one on a row about to change, and a shared one that blocks any exclusive lock until the transaction ends ([explicit locking](https://www.postgresql.org/docs/current/explicit-locking.html)) (6.7, 7.2, 8)
- **READ COMMITTED | SERIALIZABLE** — two transaction isolation levels: under the first each statement sees what was committed when it began, so rules across rows need explicit locks; under the second transactions behave as if run one at a time ([transaction isolation](https://www.postgresql.org/docs/current/transaction-iso.html)) (6.2, 6.3, 8)
- **compare-and-set** — change a value only if it still equals what you last read; publication and adoption move a pointer `WHERE current = expected`, so a stale attempt changes nothing (N5, 8)
- **commit sequence** — a counter that orders an organization's committed rows, so "everything after this point" is one comparison; the reset deletes past a state's point, and a receipt pins the record snapshot's to say which version it saw (6.7, 7.2, 8)
- **tombstone** — a small marker left where data was deleted, so readers know it is gone; S06's deletion takes the platform's tombstone path, not the reset (8)
- **audit | hash chain | chained** — the insert-only record of every governed action; its rows are chained by hash, each carrying the hash of the one before, so a deleted or altered row breaks the chain (10.6, TH7)
- **anchor | anchored** — a copy of the audit chain's latest hash written where it cannot be changed, so the chain up to it cannot be rewritten unnoticed; here it goes to Object Lock every 15 minutes (TH7, SR4)
- **Parquet** — a columnar file format, values of one column stored together, in which each imported source revision is kept immutable ([Parquet file format](https://parquet.apache.org/docs/file-format/)) (5, 6.4)
- **DuckDB** — an analytical SQL engine that runs inside the process and reads Parquet directly; the data service's typed queries run on it ([why DuckDB](https://duckdb.org/why_duckdb)) (6.3, 6.9)
- **result cache** — the data service's 2 GiB in-memory store of computed results; the reset flushes it so no take sees the last one's results, and the spike measures with it empty (6.3, 8, 11.2)
- **cold | warm** — a cold query must first fetch and decrypt its source revision from object storage; a warm one finds it already in the process (5.2, 6.3)
- **pre-rehearsal snapshot** — a stored copy of the whole database taken before each rehearsal, restorable into a new instance; the database's only backup ([DB snapshots](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_CreateSnapshot.html)) (6.6, 8, 11.1)
- **object versions | versioned bucket** — S3 keeping earlier versions of each object, so an overwrite or a delete can be undone ([S3 versioning](https://docs.aws.amazon.com/AmazonS3/latest/userguide/Versioning.html)) (8, TH2)
- **Object Lock** — an S3 setting that stores objects write-once-read-many; in compliance mode no user, the root user included, can delete or overwrite them before their retention date ([S3 Object Lock](https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-lock.html)) (TH7, TH15)
- **SSE-KMS** — S3 encrypting each object on the server with a key held in the key management service; here one key per organization ([SSE-KMS](https://docs.aws.amazon.com/AmazonS3/latest/userguide/UsingKMSEncryption.html)) (10.6)
- **key management | data key** — the cloud's key service, KMS, holding one data key per organization and the platform's own key for the operator prefix; deleting an organization's key makes its objects unreadable (6.6, 8, 10.6)
- **shredding** — deleting data by deleting the key it is encrypted under, instead of finding every copy; after delivery both organizations' keys are scheduled for deletion (10.6, 11.1)
- **7-day minimum | waiting period** — the delay KMS imposes before deleting a key, 7 to 30 days, during which the key cannot be used and the deletion can still be cancelled ([KMS key deletion](https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html)) (10.6)
- **Secrets Manager** — AWS's store for secrets; the owner credential and fAuth's database credential rotate into it after their one use, with no standing reader ([Secrets Manager](https://docs.aws.amazon.com/secretsmanager/latest/userguide/intro.html)) (TH5, 10.6)
- **owner credential** — the database's administrative credential, which creates the roles; used once by the provisioning script, then rotated into Secrets Manager (TH5, AS12)
- **keychain** — the operating system's encrypted store for secrets, which asks before releasing one; the operator's keys (AS12) live in it (6.7, TH15)
- **full-disk encryption** — encrypting a machine's whole disk, so a stolen, powered-off laptop yields nothing readable (TH15)
- **operator prefix** — the part of object storage, under the platform's own key, that holds the fixture states and the packet (6.7, 8)
- **operator views** — the platform's pages and calls for jobs, audit and the build digest, which return audit events, job states and the build digest, synthetic data only in the demo (6.7, 7.3)
- **administration group** — the platform API's operations for memberships, spaces, aliases and policy, called by an organization's administrator; the provisioning script uses it (6.7, 9)
- **digest | SHA-256** — a short hash that names exact bytes, a definition, a fixture state or an image, so changing one byte changes it; SHA-256 is the function the record names for a state's ([FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)) (8, TH2)
- **build digest | image digest** — the digest of the exact container image that runs; one for the whole recording, read by the recorder at a take's start and end ([OCI descriptors](https://github.com/opencontainers/image-spec/blob/main/descriptor.md)) (N9, 11.1, TH14)
- **manifest** — the list of what a fixture state or the packet holds; a state is sealed by a digest over its manifest and the build digest (8, TH2)
- **memento** — the design pattern that saves an object's state so it can be restored later without exposing how the object stores it; the record calls a fixture state one ([Memento](https://refactoring.guru/design-patterns/memento)) (6.7)
- **pin file** — the file in the repository, at the build's tag, that holds the fixture states' digests apart from the store they protect (TH2, AS9)
- **lockfile | lockfiles** — the files pinning every dependency to an exact version, from which the image is built (6.7, 11.1)
- **deploy log** — the cloud's record of deployments; the recorder checks that it is silent between acts 3 and 6 (N9, 6.7)
- **rehearsal log** — the packet's account of a take: times per act, sample sizes, warm or cold, and every labeled step (6.7, R9)
- **dry sign-in** — a test sign-in before a take that checks the client row, its redirect URI and `state`; a failure stops the take (6.1, SR3, 11.1)
- **rate limit | login limit** — a cap on how many requests a caller may make in a period; fAuth's login limit is raised for rehearsals (11.1, 11.4)
- **sanitised HAR export | HAR** — HAR, the HTTP Archive, is a JSON file of a browser's captured requests; Chrome's sanitised export leaves out the `Cookie`, `Set-Cookie` and `Authorization` headers and nothing else ([Chrome DevTools](https://developer.chrome.com/docs/devtools/network/reference#save-as-har)) (6.7, TH6)
- **Chrome profile | profile** — a separate set of Chrome data, history, passwords, settings and accounts, on one machine; one per actor keeps three sessions apart ([Chrome profiles](https://support.google.com/chrome/answer/2364824)) (A13, 11.1)
- **Fargate task | ECS task | ECS** — the cloud's container service, and its mode in which the cloud runs the servers; the demo is one task, one running copy of the container ([AWS Fargate](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/AWS_Fargate.html)) (D10, 11.1)
- **task definition** — the description ECS starts a task from, with its image, CPU, memory and environment; the rehearsal flag is one environment variable in the demo's ([task definitions](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task_definitions.html)) (11.1, TH1)
- **task role** — the cloud identity a task runs as; here one task role may connect as all five database roles, which is why the walls stop mistakes and not code inside the process ([task IAM roles](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task-iam-roles.html)) (5.2, 10.5, AR2)
- **credentials endpoint** — the local address from which code in a task fetches the task role's credentials; the parser module has no path to it (6.4, 10.5)
- **task replacement** — ECS stopping a task and starting a new one from the image: pull, start, registration, health check; unmeasured against N2's 30 s, which is why a supervisor restarts in place (5.2, 11.2)
- **IAM token | `rds-db:connect`** — a 15-minute credential the database accepts from the task role instead of a stored password; `rds-db:connect` is the permission to get one for a named database role ([IAM database authentication](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.html)) (10.5, AS12)
- **load balancer** — the host's edge, P1: it ends TLS and forwards requests to the one task over plaintext HTTP inside the host's network (6.6, 10.6)
- **ends TLS** — decrypts the HTTPS connection; past the load balancer, the hop to the process is plaintext HTTP (5.2, 10.6)
- **health check | grace** — the load balancer's periodic probe of its target; after a run of failures, three checks 10 s apart here, it takes the target out of service ([target group health checks](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/target-group-health-checks.html)) (5.2, 11.4)
- **Multi-AZ | standby** — a synchronous copy of the database in a second availability zone that takes over if the primary fails; the demo has none ([Multi-AZ](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Concepts.MultiAZ.html)) (5.2, 8)
- **availability zone | zone** — one of a region's separate data centres; the demo's task runs in one and its load balancer spans two (11.1)
- **region** — a geographic area of the cloud with its own data centres; prices are US East's until the head names one (11.2, 13.2)
- **CDN** — a content delivery network that serves files from places near the user; the platform's edge has one and the demo's does not (5.2, 6.6)
- **WAF** — a web application firewall that filters requests by rules before they reach the application ([AWS WAF](https://docs.aws.amazon.com/waf/latest/developerguide/waf-chapter.html)); reduced away here, an address allowlist standing in during rehearsals (5.2, 10.4)
- **allowlist** — a list of the only addresses allowed to connect, everything else refused; M29's admits the recording machine and the operator during rehearsals (10.4, 11.1)
- **NAT gateway** — the managed path that lets private resources reach the Internet without being reachable from it; the demo has none and uses the task's public address instead ([NAT gateways](https://docs.aws.amazon.com/vpc/latest/userguide/vpc-nat-gateway.html)) (11.1, 11.2)
- **egress** — traffic leaving the host; only role `worker` may reach the provider (6.4, 11.1)
- **public IPv4 address** — an Internet address charged by the hour; the cost table counts three, the load balancer's two and the task's ([VPC pricing](https://aws.amazon.com/vpc/pricing/)) (11.2)
- **vCPU | GiB | GB** — a unit of compute, and two sizes of gigabyte: a GiB is 1,024 cubed bytes and a GB 1,000 cubed; the cost table uses both (11.2)
- **gp3** — the general-purpose SSD storage class the database's 20 GiB are priced at ([RDS storage](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/CHAP_Storage.html)) (11.2)
- **db.t4g.small** — the database instance class in the cost table, whose 2 GiB of memory must also hold IAM authentication's overhead ([instance classes](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Concepts.DBInstanceClass.html)) (11.2)
- **capacity unit** — the load balancer's usage measure, priced by the hour on top of its base rate (11.2)
- **730-hour month** — the pricing convention behind every monthly line: 365 days of 24 hours, divided by 12 (11.2)
- **CloudWatch** — AWS's metrics and logs service; the cost table prices its logs and metric series ([CloudWatch](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/WhatIsCloudWatch.html)) (11.2)
- **model provider** — the hosted model that proposes app definitions from schemas and synthetic rows; only role `worker` calls it (5.1, 6.4)
- **provider tokens** — the units a model provider bills by, in and out; the cost table's last line (11.2, N10)
- **real-data disclosure** — the platform's option to let real rows reach the model provider, only in the fields an organization's policy names; off in the demo, where every row is synthetic (N8; platform N8)
- **spend limit** — a cap on what the provider's account unit may be billed, set just above the month's tokens; its error is never retried (11.2)
- **`provider_unavailable`** — the error the composer returns within 1 s when the model provider fails; during a take it pages and ends the take (6.4, 11.3)
- **WebAssembly module | WebAssembly** — a portable binary that runs inside a host program which decides exactly which functions it may call, so it reaches nothing the host does not hand it ([WebAssembly security](https://webassembly.org/docs/security/)) (5.2, 6.4)
- **system interface** — the operating-system functions, files, sockets and clocks, that a host may give a WebAssembly module, standardised as WASI ([WASI](https://wasi.dev/)); the parser gets none, only an input and an output (5.2, 6.4)
- **sandbox | parser sandbox** — the confined place the untrusted file parser runs: the WebAssembly module with no system interface, its memory cap and its fuel (5.2, 6.4)
- **fuel** — a WebAssembly runtime's budget of work, after which the module is stopped; with the memory cap it bounds a parse (TH10)
- **parser bomb** — a small file that expands to a huge one when parsed, such as a compressed XLSX inflating past its expanded cap ([OWASP file upload cheat sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html)) (10.4, TH10)
- **job | job slot** — background work, an import or a generation, whose state a person can see within 1 s; a slot is one job's place in the worker, and a full slot answers 429 (6.4, 9)
- **explanation** — in the platform, the job that sends one receipt, and only the fields the organization's policy names, to the model provider for an account of a number in words, refused if it cites a number the receipt lacks (platform 6.4); the demo's cost table budgets none (6.4, 11.2)
- **lease** — a time-limited claim on a job, 120 s here; until it runs out no other worker may take the job (6.9, 11.2)
- **jitter | jittered** — a random delay added to a retry so that many clients do not retry at the same instant ([timeouts, retries and backoff with jitter](https://builder.aws.com/content/3EumjoZascWd1oZiEgL8ORlv3qE/timeouts-retries-and-backoff-with-jitter)) (11.4)
- **403** — Forbidden: the server understood the request and refuses it; here only where the caller may already see the thing, Maya's own publish ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.4)) (9)
- **404** — Not Found, which HTTP also allows for a resource the server will not admit exists; here denied, foreign and absent are one answer ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.5)) (9, R8)
- **409** — Conflict: the request conflicts with the current state of the resource, a newer record version or an earlier request under the same key ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.10)) (7.2, 9)
- **412** — Precondition Failed: a condition the request depends on no longer holds, as with a stale patch, a changed draft or a state whose digest does not match ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.13)) (9)
- **422** — Unprocessable Content: the request is well formed but cannot be processed, as with an oversize file or a query past the grammar ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.5.21)) (9)
- **429** — Too Many Requests: the caller is being rate-limited, here by a full job slot or a queue past 20 ([RFC 6585](https://datatracker.ietf.org/doc/html/rfc6585#section-4)) (5.2, 9)
- **503** — Service Unavailable: temporarily unable to answer, here because the model provider is down; sent with `Retry-After` ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-15.6.4)) (6.9, 9)
- **`Retry-After`** — the response header saying when to try again; the browser retries only a 503 that carries it ([RFC 9110](https://datatracker.ietf.org/doc/html/rfc9110#section-10.2.3)) (9)
- **`no-store`** — the cache directive that forbids any cache to keep the response; every `/v1/` response carries it ([RFC 9111](https://datatracker.ietf.org/doc/html/rfc9111#section-5.2.2.5)) (8)
- **p50 | median** — the value half the measurements fall below: the typical case (N3)
- **p99 | 99th percentile** — the value 99 in 100 measurements fall below: the tail, meaningful only over hundreds of samples ([SRE book](https://sre.google/sre-book/service-level-objectives/)) (N3, 11.3)
- **SLO | SLA** — an objective is a target for a measured value over a window; an agreement is a promise to users with consequences if it is missed; the demo has neither ([SRE book](https://sre.google/sre-book/service-level-objectives/)) (3.2, A8, 11.3)
- **RPO | recovery point objective** — the most recent data a restore may lose, measured in time; here one rehearsal ([AWS recovery objectives](https://docs.aws.amazon.com/whitepapers/latest/disaster-recovery-workloads-on-aws/business-continuity-plan-bcp.html)) (8, TH7)
- **RTO | recovery time objective** — the longest a restore may take; unknown until S06's drill times one (8, 11.4)
- **open-loop replay | open-loop** — a load test that sends requests at a fixed rate whatever the system's speed, so a slowing system is not spared (11.4)
- **drill** — a failure or a restore staged on purpose to prove a mitigation; each is a captured scene here (11.4)
- **isolated environment** — a separate environment a restore is rehearsed into and destroyed within the hour, so the drill touches nothing live (TH7, 11.4)
- **cold start** — bringing the whole environment up from nothing, in dependency order (11.1)
- **rollout | waves** — releasing a new build in stages, each to a share of traffic, so a bad one reaches few; the demo has one build for the whole recording and no waves (11.1)
- **runbook** — the written steps an operator follows when something happens; here the take's procedure is the runbook (11.4)
- **blameless** — a write-up of what went wrong that names causes and fixes, not people ([postmortem culture](https://sre.google/sre-book/postmortem-culture/)) (11.4)
- **data classes** — the labels that decide protection and retention per kind of data: synthetic rows Confidential, credentials, sessions and keys Restricted, the recording Internal (4.2, 10.6)
- **data protection impact assessment** — the risk assessment data protection law requires before high-risk processing of personal data; due before any pilot on real data ([EDPB guidelines](https://www.edpb.europa.eu/documents/guideline/data-protection-impact-assessments-high-risk-processing_en)) (13.2)
- **soft-deleted** — marked deleted and hidden but kept, so it can be undone; fAuth's tenants are soft-deleted after delivery (10.6)
- **STRIDE** — the six threat categories the table classifies by: spoofing, tampering, repudiation, information disclosure, denial of service, elevation of privilege ([Microsoft](https://learn.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats)) (10.4)
- **trust boundary | trust boundaries** — a line on a data-flow diagram where trust changes, lettered B; each crossing gets a threat row (7, 10.2)
- **trust level** — a class of caller with a given power, lettered TL; the demo adds the canary's member, the operator's shell, the recording browser and role `reset` (10.2)
- **invariant** — a property that must hold however the environment misbehaves, with the components that enforce it named (10.2)
- **trusted computing base | trusted base** — the components that must work for the invariants to hold; here the whole binary, the reset's guard and grants and the recorder's filter ([Building Secure and Reliable Systems, chapter 6](https://google.github.io/building-secure-and-reliable-systems/raw/ch06.html)) (6.3, 6.7, 10.2)
- **blast radius** — what a compromise of one boundary can reach (10.2)
- **threat actor** — who attacks, with their capability and motive, lettered TA; an operator script bug is one here (10.3)
- **attack tree** — a goal broken into the ways of reaching it; OR means any one branch is enough (10.4)
- **residual risk** — the risk left after the mitigations, rated on the same scale (10.4)
- **WCAG 2.2** — the W3C's accessibility guidelines; N12 checks every state of the demo's components at level AA ([WCAG 2.2](https://www.w3.org/TR/WCAG22/)) (N12)
- **whole-window denominator** — dividing the window's bytes by the whole ten minutes, 1,140,000 ÷ 10 = 114,000 bytes a minute, rather than by the five minutes the included events span, which gives 228,000; act 3's patch adds the transfer-rate measure with it, a change to a calculation rather than to a style (R3, 7.1; platform 2; S02)
- **half-open window** — an interval that includes its start and excludes its end, written [10:00, 10:10), so an event at 10:10 belongs to the next window: S02's sixth event of `build-worker-17`, at 10:10:00 exactly, falls outside, which is why five of its six count (7.1; S02)

## Exercises

### After 1

The summary names three one-way doors: the organization id in every key (D2), the platform's definition and command code (D3), and the integrity rule once the packet is submitted (D7). Before reading 5.3, write for each what reversing it a month after the recording would require, and whose action it would need. Then say why the hand-added client row (D9), awkward as it is, is not on the list.

**Answer.** D2 needs our own action, but on everything stored: the organization id is in every key in every store from the first row, so reversing it means a migration of all stored data and every reference to it, and the isolation the recording showed would then describe keys the system no longer has ("keys cannot be retrofitted on camera", 12.2). D3 needs our own action too: a second implementation of the definition, admission and command code, after which every contract reopens and act 6 would have proved an implementation that no longer runs (12.2). D7 needs someone else's: the packet is with YC, so relabeling a step would take the recipient's action, not ours ("a packet cannot be relabeled after YC has it", 12.2). Two doors are one-way because of what a reversal would touch, the third because the reversal is out of our hands. D9 is one row and one redirect URI in fAuth's database: replacing it, with the refresh-cookie bridge or a future client API (13.2's `POST /manage/oauth-clients`), is our action on one adapter's input and touches nothing the platform stores, which is why 5.3 marks it two-way (1, 5.3, 12.2).

### After 2

Write a fourth shortcut in section 2's form, for act 5, share and reuse: the fastest way to make Jordan's reopening of Maya's case look right on screen, other than section 2's third, which already has one session play everyone. Then name what it leaves untested, by the goal or requirement it fails, and the observation in the packet that would expose it.

**Answer.** One version: give Jordan a copy of Maya's case under a new id; the screen shows the same findings. A copy is a different work id, so G3's "reopens Maya's case by its work id at its pinned publication with its records" fails, and so does R6's same id, records and pin. It also outlives revocation, because revoking Jordan's grant on Maya's case does not touch his copy, so R6's "revocation denies later results" cannot be shown, and a concurrent edit cannot conflict because the two cases share no record (G3's visible conflict). The packet would expose it twice. Act 5's "both work ids" must be Maya's own id, reopened under Jordan's session, and his new one for `ops-runner-04`; with the copy, the case Jordan opens carries an id Maya never had. And no denial follows the revocation (5 act 5's "Captured"; 7.1 act 5; S04). Any answer is right that names a mechanism replaced by its appearance and the identity or denial that the evidence would then lack (2).

### After 3.3

It is the morning of a rehearsal and sign-in is broken, in one of three ways: (a) fAuth's service is down; (b) the provisioning script inserted the client row with a typo in its redirect URI; (c) the head learns that fAuth at the pin cannot hold a registered client at all. From section 3 alone, name the assumption each case breaks, if any, and what you would do. Then derive what the assumptions only imply: for (a), what must already be true of the design for A12's "only act 1 changes" to hold; for (c), which goal A13's fallback strains, and why. Check (a) against 6.9 and 11.1, and (b) against SR3 and 11.4, when you reach them.

**Answer.** (a) breaks the first half of A12, fAuth opening act 1: wait for fAuth, or switch before the take to A12's fallback, the platform's minimal login behind the same adapter; either way only act 1's scene changes. For "only act 1 changes" to hold, a session must not need fAuth once it exists: the platform keeps its own sessions and checks fAuth's tokens without calling fAuth. It does: sessions are PostgreSQL rows that live 12 h, tokens are verified offline against the cached key set, and the fallback is switched before a take, never during it (5.1, 6.1, 6.9, 11.1; 11.4's fAuth row: act 1 waits, later acts continue). (b) breaks no assumption: A13 holds, since fAuth can hold the row, and the row is simply wrong, so nothing in the design moves. Correct the row by hand and prove sign-in before the camera rolls; the record's check is the dry sign-in, which stops a take until it passes (SR3, 6.1; 11.4's row for the provisioning script's output), and D9 being a two-way door is why a hand fix is enough. (c) breaks A13: its fallback is the same-site refresh-cookie bridge behind the same adapter, so act 1's mechanism changes and D9 would be revisited. The goal it strains is G7: the bridge is a session contract the platform never ships, so either the demo signs in in a way the platform does not, or the platform carries it for one demo, and 12.2 rejects it for that reason (12.2's table, 12.3). The three mornings look alike, "sign-in is broken", and the assumptions separate them: one has a fallback, one is an error to correct, one moves a decision (3.3, 5.3, 12.2).

### After 4.2

4.2's basis is ten rehearsals, each one whole-run take filmed at most twice, and a 30-minute rehearsal holds about 360 interactions: about 200 six-view refreshes (1,200 operations), 40 navigations, 20 commits, and edits and uploads for the rest. Rebuild the 360 and the 1,200, and work out how many edits and uploads remain. Then, before reading 11.3, decide over how many rehearsals you would pool navigations and commits to report a p99, and count the samples that would lie above the p99 in each case, compared with the 200 refreshes of one whole-run take.

**Answer.** 30 minutes is 1,800 s; one action every 5 s is 360 interactions; 200 refreshes times six views is 1,200 query operations; 360 − 200 − 40 − 20 = 100 edits and uploads. A p99 leaves one sample in a hundred above it, so one whole-run take's 200 refreshes leave 2 above it, enough to say something, while its 40 navigations or 20 commits leave less than one, so their "p99" would just be the slowest sample. Pooling ten rehearsals, one whole run each, gives 400 navigations, 4 above the p99, and 200 commits, 2 above, which is what 11.3 does; the refresh needs no pooling because one take of the whole run already holds 200 (4.2, N3, 11.3). The counts are per whole run: a take of one act holds only that act's share of them.

### After 5.2

Rebuild a take's burst from 5.2's rules: how many query operations one filter change puts at the data service, how many run and how many wait, how many waves, and how long at 100 ms a warm operation. Then redo it for a version of Spend Review with eight linked views during a scene where two actors act at once: the operations, the waves, the time, any 429s, and the warm operation time at which N3's 2 s would stop holding.

**Answer.** One actor, six linked views: six operations; the data service admits two of an organization's at once, so 2 run and 4 wait, three waves, 0.3 s at 100 ms, inside N3's 2 s (5.2); the waves are why 6.3 puts the ceiling at a warm operation of about 650 ms, three waves inside 2 s. Two actors, eight views: 16 operations in one organization, 2 running and 14 waiting; 429s begin only past 20 waiting (5.2; platform 6.3), so none is refused; eight waves of two, 0.8 s at 100 ms, still inside 2 s; and N3's 2 s stops holding at a warm operation of about 2 s ÷ 8 = 250 ms. The envelope's burst of 24 is different in kind: 22 waiting is two past the 20, those two are the 429s, and that is why four simultaneous actors are not a take (5.2, 6.3, 11.2).

### After 5.2

5.2 has the supervisor restart the binary "inside the load balancer's grace of three failed checks 10 s apart". (a) From those two numbers, how soon after a crash can the third failed check come? (b) Predict what the take sees if the binary takes 40 s to come back. (c) Act 4's restart drill falls between acts 3 and 6, where N9 allows no deploy. From N9's own wording, decide what a change must alter to count as a deploy, and so whether the drill is one; then design the check that would tell a restart from a deploy in the evidence, and compare it with 6.7 and TH14 when you reach them.

**Answer.** (a) Checks come every 10 s, so the first failure lands up to 10 s after the crash and the third 20 s after the first: between about 20 and 30 s after the crash the target is taken out of service (5.2; the load balancer's failure threshold). (b) At 40 s the target has been taken out of service, so the load balancer stops sending it requests and Maya's reload fails until the binary is back and the checks pass again (11.4's first row: the target re-registers on the next check); the case reopens later than N2's 30 s, and a miss of N2 ends the take (11.1, step 5), so act 4 is retaken from `s3`, and the acts after it with it (8). One more consequence is an inference the record implies without stating it: 5.2 puts the restart inside the grace so that it triggers "no task replacement", so a restart past the grace may bring the task replacement as well, the sequence nobody has timed against 30 s (RK8). (c) N9 asks for one build digest for both apps and no deploy between acts 3 and 6, so what it forbids is a change of the image that runs. A restart, in place or even as a new task, runs the same image digest, so it is not a deploy, as 6.7 says outright: "a task start on the same image digest, act 4's restart drill, is not a deploy". The check follows from that definition: read the task's image digest at the take's start and end, and the deploy log between them. A restart leaves both unchanged; a deploy changes the digest or writes to the log, and either fails acceptance. That is TH14's M28, run by the evidence recorder (6.7, N9, TH14).

### After 6.4

Act 2 must capture one oversize refusal (S06). The platform record's example of a bomb is a 9 MiB XLSX that inflates past 100 MiB (platform 7.1). Explain why a 9 MiB file could never test the demo's expansion cap. Then, before reading 7.1's act 2, predict what a 4 MiB file that inflates past 50 MiB tests instead: which of A6's limits fires, and where; what stops and what survives; what the job and the recording show. Finally say what the module's 2 GiB memory cap is for, if a limit fires long before it.

**Answer.** A6 caps an XLSX at 5 MiB compressed and 50 MiB expanded, and a file's compressed size is known before anything opens it, so a 9 MiB file is turned away at the size check and never expands: it tests the byte cap, not the expansion cap (appendix C confirms it in TH10's M19: "the byte caps refuse before parsing"). In the platform, whose compressed cap is 10 MiB (platform A4), the same file does reach the parser, which is why it is the platform's example. The 4 MiB file passes the byte cap, so the parser starts inflating it inside its WebAssembly sandbox and meets the 50 MiB expanded cap: the parse stops there and the file is refused, the job carries the reason, the import fails visibly, the refusal is captured, and the process and the take carry on (7.1 act 2; 11.4's parser row). The 2 GiB memory cap never fires in this case: it is the backstop. If a parse ever got past the expanded cap, the runtime would still stop the module at 2 GiB, and the task's 8 GiB is sized so that 2 GiB fits beside everything else the process holds, so even then the module, not the process, would die (7.1, 11.2; TH10's M19). What the file tests is the bounded parser that is the reduction's seam (6.4) and the property one process makes critical: a bad file costs a refusal, never the process (TH10).

### After 6.7

The reset keeps sessions and bumps the membership epoch. Predict what would change if it deleted every session row instead and left the epoch alone: for the take procedure, for fAuth, for role `reset`'s grants, and for a tab still open from the previous take.

**Answer.** The procedure: every actor would have to sign in again through fAuth's page after every reset, instead of once within the idle hour (11.1, step 3, which relies on "the reset keeps sessions"). fAuth: more sign-ins per rehearsal push against its login limit, which the cold start already has to raise (11.1; 11.4's limiter row). The grants: role `reset` would need a grant on the session table, which SI7 forbids ("never deletes audit or sessions", enforced by `reset`'s grants), so the most dangerous role in the demo would reach identity rows. The open tab: it would be denied at its next request because its session is gone, the same safety the epoch bump gives, bought with re-sign-ins. The epoch bump gets that safety without touching sessions: every open tab re-authorizes within 5 s against the restored membership rows and no session is lost (6.7, 8, TH9).

### After 7.2

In act 6's lost response, `ik-1` is posted twice, both times with expected version 0, though after the first post the record is at version 1. Suppose P4 compared the expected version before looking for the stored outcome: predict what the second post would return and what Maya and the packet would then show, and say what finding the outcome first protects. Then compare the two ways a client can recover from a lost response, retrieving the outcome by key first (section 9's rule) or re-submitting under the same key (the path the script forces): what would each leave in the packet, and which part of act 6's claim does only the second exercise?

**Answer.** Version first, the second post would fail: its expected version 0 no longer matches the record's version 1, so P4 would roll back and answer 409 with the recovery view, as it does for the stale edit under `ik-3` (7.2, step 5). Maya would be told that her own decision conflicted with itself, and the packet would hold a 409 for a command that had in fact committed: the retry would look like a conflict, and G4's duplicate-safe retry could not be shown. Finding the stored outcome first is what makes a retry safe whatever the client believes: the first post stored its outcome under `ik-1` in the same transaction as version 1 (F7), so the second finds it before any version check and returns version 1 (F7, F8; step 4), while authorization still runs again (F4), because a key names a command, not a permission. Retrieving first would leave one POST and one GET by key: it shows that an outcome is stored, but the server never sees the command twice. The forced re-submission leaves two POSTs under one key against one record revision, one audit event and "the attempt logged apart" (7.2, step 4): it exercises the duplicate path itself, the outcome found before the version check, which is G4's "an `ik-1` retry yields one revision and one audit event". That is why the record calls it "the stronger proof" and labels it simulated (7.2, 9, G6).

### After 8

Section 8 lists `s0` and then `s2`, and appendix A gives the reason: act 1 changes nothing a take restores. Test that claim instead of taking it: list what act 1 leaves behind (7.1) and say why, under the reset's rules (6.7, 8), none of it matters to a take of act 2; then name a change to act 1 that would force an `s1` into existence. Finally, argue why a fixture state is captured from a clean take's act end rather than written by hand as the next act's starting rows, and name the two checks that stop a state altered after its capture from being restored unnoticed.

**Answer.** Act 1 leaves one session and one audit row and nothing else (7.1, act 1). The reset keeps every session and every audit row and deletes only rows committed after the state's point, so restoring `s0` before act 2 removes nothing act 1 made, and act 2 starts from `s0` too (8; 11.1, step 2). An `s1` would be needed the moment act 1 committed a row a later act depends on, for example a space created on camera instead of by the provisioning script (6.7): a reset to `s0` deletes every row past `s0`'s point, so that row would be gone before act 2. A state captured from a clean take holds only rows the build's own code wrote, a revision 5 that admission accepted, a publication 1 the control plane wrote after Sarah's approval, under the build digest the state carries (8). Rows written by hand could hold something no mechanism produced, a publication with no approval behind it, and restoring them would put a state in front of the camera that no clean take reached. The reset itself is labeled, its audit event entering the rehearsal log as seeded state (9; appendix A's labeled step), but the act filmed after it is accepted as working, so it must start from rows a real take produced. The checks: the reset recomputes the state's digest, over manifest and build digest, against the pin file kept apart in the repository and refuses a mismatch with 412 (M3), and the recorder diffs each restored manifest, publication and approval digests included, against its capture (M4) (TH2, 8, 9).

### After 10.4

The canary suite must get a 404 for every replay, each paired with the owner's 200 for the same identifier in that take (R8, SR2). Say in one sentence which property the suite proves. TH4's preventative half already puts row-level security on every table: argue why the row still needs this detective half, naming one thing the suite can catch that row-level security cannot. Then describe two different broken setups, one in the build and one in the suite's own script, that a suite of bare 404s would pass and the paired suite would fail.

**Answer.** It proves that nothing of Northstar's, data or metadata, reaches a session of another organization (G5, N7): it is TH4's detective half, M8, where SI1 is at stake, the platform's scope invariant that every read and write is scoped by the verified session's organization, never a request value (platform 10.2). Row-level security filters rows; it cannot see the rest of an answer. A handler that answers 403 where it should answer 404, or names the app in an error, tells the canary that Northstar's identifier exists without returning a single row, which is why R8 asks for denial "before data or metadata" and 5.1 for 404 rather than 403; and a policy missing from one table is a gap the preventative half cannot report about itself. The suite checks whole answers from outside, every take, so it sees both, which is why 10.4 gives every High row a preventative and a detective half. A broken build: a change that makes a route answer 404 to everyone, its owner included; the canary's 404 then proves only that the route is broken, and the owner's missing 200 fails the take. A broken script: replays built from the previous take's identifiers, which the reset deleted, so every replay is 404 because nothing exists; again no owner 200 in this take. In both a bare 404 cannot be told from a refusal, and the paired 200 is what proves the identifier was real and reachable by its owner at the time (R8, SR2, TH4; 11.1, steps 4 and 6).

### After 10.6

Rank three leaks from least to most damaging, and for each say what it yields and what bounds it: (a) a copy of the `oauth_clients` row taken from fAuth's database; (b) fAuth's platform key, leaked a week after provisioning; (c) the operator's machine, taken during act 5. Then explain why the platform key is the first of the operator's keys to be deleted.

**Answer.** Least to most damaging: (b), (a), (c). (b) Nothing, if the rule was kept: the platform key creates tenants (appendix A, AS12) and "dies once the tenants exist" (10.6, M30), so a week after provisioning there is no key left to leak; it ranks first only because the rule held, and a key kept by mistake would be the power to create tenants at fAuth. (a) Next: the row holds no secret, only the client's registration and its one redirect URI (D9, 6.7). A code can be sent only to that URI, and redeeming one needs the code verifier the platform made for that one sign-in, which is never in the row (PKCE), so the copy signs nobody in; what remains is AR4, a wrong row added by hand, caught by the dry sign-in (M6). (c) Most: the machine holds every key still alive, the actors' sessions in their Chrome profiles, the raw export and the pin file at once (AS13, TH15), which is any principal in either organization and every operator power. It is bounded by M30 (fresh profiles destroyed after the last take, the raw file hashed, filtered and deleted in one step, full-disk encryption, the machine off the network except while a rehearsal runs), made visible afterwards by M31's anchored manifest, and the rest accepted (AR3). The platform key goes first because creating the tenants is the provisioning script's first fAuth step (6.7), so it is the first key whose job is over; the others still have work, the owner credential until its one run, fAuth's database credential until its one insert, the management keys until delivery (10.6). A key that will never be used again is only risk ("single use where possible", AR3).

### After 11.1

Diagram 03 runs Spend Review's command with no control plane, P3, on it, yet the workspace server checks every decision against the pinned publication's record schema, read from the publication row (F5). Argue why reading the row is as safe as asking P3, from who writes a publication and what it names (6.2). Then predict what migration step 4, one process to four services, would have to add if P2 asked P3 for that schema on every command instead, and which failure would then stop a decision that the design as written survives.

**Answer.** Only role `control` writes a publication, inside the platform's SERIALIZABLE transaction, and a publication names one exact approved digest (6.2); a case stays pinned to its publication until an explicit adoption (R6). So the row cannot hold a schema P3 did not admit and publish, and it cannot change under a pinned case: asking P3 again would return the same answer, at the cost of a call, and the run path needs the database, not the control plane (7.2). If P2 asked P3 on every command, step 4 would turn that in-process call into a network hop on every decision, with its own request type and a principal carried across it, the things 11.1 says step 4 needs from D1, and the control plane's service would join the hard dependencies of every published app. Today the four roles share one process, so P3 cannot fail apart from P2; after step 4 it can, and a control-plane outage or deploy would then stop decision forms, where the design as written loses only what P3 itself does, admission and publication (6.2). That is what 7.2 means by "the control plane stays off the run path at migration (G7)" (7.2, 11.1).

### After 11.2

Rebuild the task's 8 GiB from its five parts, and the $85 Fargate line from the task's size and unit prices. Then rebuild the month of about $170 from its five lines and its $17 a rehearsal. Finally predict both numbers if the head asks for twenty rehearsals a month instead of ten, assuming the spike and margin stay at 40 generation attempts. Which line is the driver, and does it move?

**Answer.** 1 GiB for the binary, 2 × 1 GiB for two DuckDB operations, the parser module's 2 GiB cap, the 2 GiB result cache and 1 GiB of headroom: 8 GiB. Fargate: 2 vCPU × 730 h × $0.04048 = $59.10, plus 8 GiB × 730 h × $0.004445 = $25.96, so $85.06. The month: $85 + $26 + $22 + $24 + $11 = $168, "about $170", and $16.80, about $17, a rehearsal at ten a month, $1.10 of it tokens. Tokens: 120 attempts are 2 attempts × 2 acts × 20 takes = 80, plus 40 for the spike and margin, at 8,000 tokens in and 3,000 out, $0.032 + $0.06 = $0.092 an attempt. At twenty rehearsals, 40 takes: 160 + 40 = 200 attempts, $18.40, so the month is about $175 and a rehearsal about $8.80. The driver is the Fargate line, and it does not move, because hosting runs whether or not anyone rehearses (11.2).

### After 12.2

Make the strongest case, in three sentences and with this record's own numbers, for recording on the founder's machine or a single virtual machine instead of the ECS task (D10's alternative): what it would save a month and a rehearsal, and what the scenes would lose by it. Then argue the other side from G7 and from what the founder's machine could never measure, and, before reading 13.2, predict the one measurement that should settle the host, and which result would move the demo off the task.

**Answer.** For: on the founder's machine about $157 of the $168 month disappears, everything but the $11 of tokens, since a take's marginal cost is its tokens, "all the founder's machine (D10) costs" (11.2), so about $15.70 of each rehearsal's $16.80; a virtual machine needs no ECS account (12.2's table); one operator (A3) has one fewer system to provision and watch; and the restart the recording times is already a process restart under a supervisor (5.2), so the scenes lose nothing a task replacement would have shown. Against: the platform's own deployment is ECS on Fargate in one region, so a demo on the task is the platform's first deployment and its migration starts there, while a laptop or a virtual machine is a deployment to redo (G7). And the number the host question waits on, how long a full task replacement takes against N2's 30 s, can be measured only on the task: 5.2 leaves it unmeasured and times one apart, and the founder's machine "cannot time a task replacement" (12.2's table). So the deciding measurement is that one timed task replacement: if it meets N2's 30 s the task stays; if it misses, the host becomes one virtual machine, which is 13.2's current choice (5.2, 11.2, 12.2, 13.2).

### After 13.3

The second-app objection is answered by one fact: policy is per organization. Make the founder's best case for letting Maya publish Spend Review herself under owner publication, to save ten seconds; then show what that change would break elsewhere in the recording and what it would prove instead. Finally, if the provisioning script set owner publication by mistake, say what a check must compare the policy row against to catch it before the camera rolls, and why checking once, right after provisioning, would not be enough.

**Answer.** For: Spend Review records internal decisions about synthetic expenses and pays nobody (7.2, step 6; 6.8's "no connector or external action"), so the founder can argue that a wrong decision costs little, that review adds only delay, and that ten seconds matter in a five-minute act. What it breaks: Security and Finance share Northstar's one policy row (6.2; 7.2, step 2), so owner publication for Spend Review is owner publication for the investigation app too; Maya's own publish in act 4 would succeed, and R4's refusal, the scene that proves independent review, would disappear (6.2's "act 4 loses its refusal"; RK4). What it would prove instead: that an administrator can set an owner-publication policy and the platform honours it, a claim about configurable policy rather than about a reviewer approving exactly what runs; and the partner would see an app nobody else approved, the very objection 13.3 answers. The check: the policy row against the provisioning script's manifest, the record of what the script meant to set, since the row alone cannot say it is wrong. Once is not enough, because the row can change after provisioning: the script is rerun when its output is wrong (11.4), and every reset restores rows from a captured state, so a wrong policy captured once comes back with each reset until the states are recaptured. That check is the pre-take assertion, reading each organization's one policy row against the manifest before every take (6.2; M10, as RK4 cites it), so a wrong kind stops the take before the camera rolls (R7, 13.3).

## Task map

The controlling definition is the App Platform definition, which `PRODUCT-BRIEF.md` restates for the demo and the record cites as §16 (the six acts), §18 (the completeness test) and §19; `SCENARIOS.md` turns it into the fixtures S01 to S07, and `BUILD-HANDOFF.md` lists the eight legitimate reductions, each with the seam the platform must keep.

| Ask in the brief | Where the record answers it | What to learn from it |
|---|---|---|
| §16 act 1, enter the company workspace: click and `/security` resolve to one space, only permitted items show, an unknown command creates nothing | R1, 5 (act 1), 7.1 (act 1), 6.1, D4, D9 | Start with the platform, not the app; the first evidence is an absence, nothing created and no model called, which only an audit row can show. |
| §16 act 2, bring evidence and describe the job: a CSV by drop and an XLSX by keyboard Upload, interpretations confirmed, revisions pinned, nothing published | R2, A6, 6.4, 7.1 (act 2), TH10 | An import is untrusted input until a person confirms its meaning; real caps make the refusal on camera a test rather than a prop. |
| §16 act 3, form and transform a useful app: six linked views labeled retrieved, adapted or generated, a meaningful change as a patch with preview, diff and undo | R3, 5 (act 3), 6.4, 6.5, 7.1 (act 3), D5, D8, TH11, TH12 | A meaningful change is a calculation that changes, the whole-window denominator, shown as a semantic diff; a restyle would not count. |
| §16 act 4, publish and run: the reviewer approves the exact draft, the author's own publish is refused, live work with receipts survives a reload | R4, R5, 5 (act 4), 6.2, 7.1 (act 4), N2 | The refusal is as much evidence as the approval, and a restart filmed and timed turns the design's weakest point into a measurement. |
| §16 act 5, share the work and reuse the tool: the same case reopened by a colleague, new work with its own identity, revocation | R6, G3, 5 (act 5), 7.1 (act 5), 6.3 | Sharing gives a second person the same identity under a new grant, reuse gives a new identity with new bindings; keep the two apart and both can be tested. |
| §16 act 6, create a different app: a queue, a detail view and a decision form through the same path, a duplicate-safe retry, a stale-edit conflict, the return to Security | R7, G2, G4, 7.2, 8, 9, N9, D3 | A second app proves the platform only if nothing changed between the two: one build digest, a silent deploy log and the platform's own command code. |
| §18: at least two apps with different purposes exercise one creation, publication, access and saved-work model, with no developer changing platform code | G1, G2, N9, D3, 7.2, 13.3 | Generality is observable only as an absence: no branch on a tenant's name and no deploy between the apps. |
| §18: a second user completes supported work independently | G3, R6, 7.1 (act 5) | Independence is a separate work id with empty findings beside the first user's untouched totals. |
| §18: saved work reopens with its context intact | R5, R7, N2, 6.1, 7.2 (step 7) | Context that survives a reload and a restart lives on the server; the browser only drops late answers. |
| §18: revoked permission prevents further access | R6, SR6, 7.1 (act 5), 7.2 (a revocation racing a paused command), TH9 | Revocation is an epoch checked on every request and at delivery, so queued work is denied too; a download already made cannot be recalled. |
| §18: a changed source or calculation produces an understandable state | R3, R6, 6.2, 9 | Every change makes a new identity, a revision or a digest, so old approvals and old work cannot silently follow it. |
| §18: a published app stays usable when a new draft or generation attempt fails | R4, N4, 6.4, 7.1 (act 4), 11.4 | Keep the model off the published app's path; its outage then ends a take but never breaks published work. |
| §18: a reused app binds to the new user's permitted data | G3, R6, 7.1 (act 5) | Reuse copies the app, never the evidence: new bindings and new totals. |
| §18: organization boundaries hold | G5, R8, SR2, D2, TH4, 7.2 (step 8) | Isolation is proved by an adversary you script and pair with the owner's success, every take. |
| §19: a synthetic demo is not evidence of adoption | 3.2, A10, N8, 13.2 | Say what the evidence cannot prove: a synthetic recording shows mechanisms, not customers. |
| S01 platform home and meaningful creation: one id for click and slash, permitted items only, labeled creation, both import paths and the keyboard path | R1, R2, R3, N12, 5 (acts 1 and 3), 7.1 | Accessibility is an acceptance path, not a finish: the keyboard path imports and navigates like the mouse. |
| S02 Northstar arithmetic and source interpretation: the half-open window, 1,140,000 bytes, 240,000 matched and 900,000 unmatched, the whole-window rate, late epochs dropped | R2, R5, N6, 6.5, 7.1 (acts 2 and 4) | Pin meaning in the fixture: integers recomputed on the server, a half-open window and a whole-window denominator leave no room for a plausible wrong number. |
| S03 transform, preview and publication: a patch with a semantic diff, undo as a revision, a stale patch refused, approval of the exact draft, self-publication refused | R3, R4, 6.2, 7.1 (acts 3 and 4) | Approve a digest, not a draft number, so that any later change voids the approval. |
| S04 same work, new work and version adoption | R6, G3, 7.1 (act 5), 9 | Two identities, one for continuing and one for starting, each with its own grants and its own revocation. |
| S05 a second app with writable records | R7, G4, 6.3, 7.2, 8, 9 | One transaction holding outcome, version, epoch and audit gives idempotency and visible conflicts together; the order of the checks is the design. |
| S06 failure and adversarial cases | N4, N7, R8, 10.4, C, 11.4 | Each hostile stimulus becomes a test that captures the server's denial and the absence of effect, never a model's refusal. |
| S07 evidence packet for a working demo | R9, G6, D7, 6.7, SR4, 13.3 | The packet, not the video, is the evidence: pins, digests, receipts and outcomes as received, every simulated step labeled, the manifest anchored. |
| Reduction: two synthetic organizations and manually provisioned accounts | 6.8, 6.1, 6.7, D6 | Shrink the number of tenants, never the tenancy: real membership replaces the script through the same adapter without touching authority. |
| Reduction: one operated deployment with small stores | 6.8, 6.6, D1, D10, 11.1 | Collapse deployments, not boundaries: database roles keep the write grants when the network hops go, and the organization id in every key lets the stores split later. |
| Reduction: small CSV and XLSX fixtures | 6.8, 6.4, A6 | Lower the limits and keep the pipeline: the same bounded parser, its caps raised when measured. |
| Reduction: two apps and a small trusted component library | 6.8, 6.5, D5 | A library change is a release made before the build digest is taken, never a change during the recording. |
| Reduction: one configured model, or synthetic composition fixtures in an early spike | 6.8, 6.4, D8, TH12 | A stand-in for the model must be labeled where it is made, in the admission report, so it can never pass as generation. |
| Reduction: manual policy and retention administration | 6.8, 6.2, 6.7, D6, RK4 | Administration by script still goes through the platform's own administration API and its audit, and a check before every take keeps the script honest. |
| Reduction: no simultaneous coediting | 6.8, 6.3, 3.2 | Expected versions and visible conflicts prove the property co-editing would need, no silent overwrite, at a fraction of the cost. |
| Reduction: no live connector or external action | 6.8, 6.3, 7.2 (step 6) | Keep internal records and external effects apart: a "reviewed" decision pays nobody. |

## Topics

- design-docs
- decisions
- threat-modeling
- authentication
- multi-tenancy
- data
- operations
- requirements
- processing
- supply-chain
- diagrams
- frontend
