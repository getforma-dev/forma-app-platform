# Security review: the Northstar demo record

Reviewer: a fresh security reviewer with no prior knowledge of the project. Date: 2026-10-22.

Scope as given: `design.md` sections 7 and 10, appendix D and section 4's SR1 to SR6, against `reference/CHECKLIST.md` R6, R16 and R21 and `reference/GUIDE.md` section 3 item 10; the platform record's section 10 and appendix D read so that each "holds unchanged" claim could be judged on one host, one process and one operator; diagram 03 read from `output/pages/page-20.png`. Nothing else was read (no briefs, no STATUS, no input folder, no section 11), so a control that lives only in section 11 may be missed; where a finding says "unstated" it means unstated in the reviewed sections.

What diagram 03 shows, for the trace findings below: EE1 the member's browser and P8 inside B1 ("the Internet: the recording browser"); P1, P2, DS4, DS1 and P10 inside B2 ("the host's network: the edge, the one process, PostgreSQL, the operator's script"); P3 inside B3 and P4 inside B4 as role boundaries; arrows F1 to F13, with F12 the browser's exported network log into P10 and F13 the operator views from P2 into P10. Not drawn: P9, P11, EE2, EE3, EE4, DS2, DS3, DS5, P5 to P7, B7, and any arrow for the reset or the fixture states.

## Counts

| Severity | Count |
|---|---|
| Blocking | 0 |
| Major | 12 |
| Minor | 12 |

No finding blocks on its own: the structure the guide asks for is present (scope, trust levels, invariants with enforcers, entry points, assets, actors and goals, full threat statements, an attack tree, a data dictionary, per-secret rotation, a security-event table, fail-closed per control, accepted risks with an owner, SR rows with verification, a maintenance trigger). The majors are where the record says a platform row "holds unchanged" and it does not on one process or under one operator, where a central claim is stronger than the mechanism, and where the proof the demo rests on (the canary suite) can pass vacuously as specified.

## Findings

Each finding: item, location, severity, problem, fix. M-ids and TH-ids are the demo's unless prefixed "platform".

### F1. "Every other platform row holds unchanged" is false for platform TH8: the service token is gone

- Item: R6 (existing controls separate from new mitigations), guide 10.4; R16.
- Location: 10.4 last paragraph ("Every other platform row holds unchanged"); 10.5 second paragraph ("the platform's service token (platform M16) is not minted"); platform appendix D TH8.
- Severity: major.
- Problem: platform TH8 (a compromised workspace, or a handler skipping the authorizer, forwards a principal it never verified) is mitigated on the platform by M16, a 5-minute KMS-certified service token verified by P3 and P4. The demo removes M16 and substitutes "a typed value in process, constructed only in the authorizer's module". That is a different control with a different strength: module privacy stops a logic bug, not code running in the process, and the callee's re-read of membership (F7) is the only remaining check. The row's existing control, mitigation and residual all change, yet the record carries it as unchanged and only alludes to it in "the residual: platform TH1 and TH8 to TH10 keep row-level security ... without the wall of separate nodes (AR2)".
- Fix: write platform TH8 as a demo row (TH15 in appendix D): elements P2, P3, P4, F5, F6, B3, B4; existing control none; mitigation M29 prevent: the principal type is constructible only in the authorizer's module, the callees re-read membership and the epoch in their own transactions (F7), and `workspace`'s pool holds no grant on publications, records or Parquet; M30 detect: the canary suite replays every route sessionless and with a forged principal id in the body, and a policy test asserts the grant table per role on every build. Rate Low likelihood (needs a flaw in the authorizer's module or a process compromise), High impact, status proposed, residual Medium under AR2 (a compromise of the process is every role). Say in 10.5 that M16 returns when roles split (G7) is a seam, not a control.

### F2. Platform TH4 does not hold unchanged: the sandbox now shares the process with every role, and demo TH10 drops the elevation half

- Item: R6, guide 10.4 (High rows carry a preventative and a detective mitigation; impact rated against what the compromise reaches).
- Location: appendix D TH10; 6.4 ("P6 stays a WebAssembly module"); 5.2 ("the parser's child process"); D1 ("the parser a sandboxed child"); 10.1 ("the release path (platform TH6 unchanged)" and the unchanged list for TH4).
- Severity: major.
- Problem: on the platform, a parser escape lands "beside the provider key and every organization's revisions" on a worker task (platform TH4, High/High). In the demo the same escape lands in the one process that holds all five database pools, both organizations' KMS contexts, the client secret and the provider key through the one task role, and the reset role. That is strictly worse than the platform's, but demo TH10 is rated Denial of service only, High/Medium, "inside one organization", and no row carries the escape. The record is also inconsistent about the mechanism: 5.2 and D1 say a sandboxed child process, TH10 says "a WebAssembly module inside the one process". The two have different blast radii (a child with no capabilities and its own address space versus a runtime boundary inside the address space of every role).
- Fix: decide the mechanism and say it in one place (6.4). Recommend the child process: `fork`/`exec` of a minimal runner with no network, no credentials endpoint, a memory limit and a kill timer, bytes on a pipe (the platform's "two streams"). Then split TH10 into DoS (as written, High/Medium) and an Elevation of privilege row: "a parser exploit (TA3's file) escaping P6 inside the one process can run code with every role's pool, both data keys, the provider key and the client secret, which leads to both organizations read and the operator's secrets taken, negatively impacting AS3, AS4, AS5, AS12"; Low likelihood (a runtime flaw), High impact; prevent: the child process with the runtime's caps, `seccomp` or the platform's equivalent, no access to the task's credentials endpoint (block `169.254.170.2` from the child); detect: a killed or crashed child audited (platform M9), both parsers fuzzed in CI; residual Medium under AR2. Remove "platform TH4 unchanged".

### F3. Platform TH11 does not hold unchanged: the owner credential sits in the operator's keychain and there is no breakglass

- Item: R6; R21 (operator access: breakglass with its audit reader).
- Location: 10.2 TL10 and AS12 ("the database owner credential"); 10.4 TH5 M9 ("the owner credential creates the roles once, then rotates"); 10.5 ("No two-person rule and no breakglass"); AR1, AR3; platform appendix D TH11 (M22: "`owner` only by breakglass").
- Severity: major.
- Problem: platform TH11 (audit rewritten by an operator with database credentials) is rated Low/High with residual Low (AR9) because `owner` is reachable only by a sealed breakglass role whose CloudTrail an outside reviewer reads. In the demo the owner credential is a standing key in the operator's keychain (AR3), used routinely by P9, with no breakglass and no second reader; the detective control that remains is the 15-minute Object Lock anchor (M13), which the record cites only against loss (TH7). AG6 names TA9 but no demo row covers the operator rewriting audit, and TH5's statement ("reset, revoke or set policy alone and deny it") does not reach the audit table. "Then rotates" also leaves open what the rotated credential is and who holds it; if the operator keeps it, nothing rotated.
- Fix: re-rate platform TH11 for the demo as Medium likelihood (a standing credential used every rehearsal, one person), High impact, residual Medium under AR1 and AR3, and write it as a demo row or widen TH5 to name AS7 and SI5's audit chain. Mitigation: the owner credential used once to run the role-creation script, then its password rotated by RDS into Secrets Manager with no reader (the operator re-grants itself access only through a recorded just-in-time step); P9 and P11 never connect as owner afterwards; the anchor job runs in the demo (say so in 10.6, it is a platform M22 element the demo must build) and P10 files the chain verification against the anchors with every packet, so the audience, not the operator, is the audit reader (see F11).

### F4. Platform TH14 does not hold unchanged: the edge limiter went with the WAF and the host is public

- Item: R6; guide 10.4 (likelihood rated against existing controls).
- Location: 6.6 and section 7 opening ("[reduced: the platform's edge, no CDN, no WAF]"); 10.1 risk profile ("a host nobody is told about"); 10.4 unchanged list; platform appendix D TH14 (M28: "100 requests in 10 s a client at the edge").
- Severity: major.
- Problem: platform TH14's first preventative, the per-client edge limiter, is a WAF rule; an Application Load Balancer alone does not rate-limit. The demo removes the WAF, keeps a public IPv4 (appendix B) and relies on the in-process limits (8 queries, 2 an organization, 20 waiting, 429) and fAuth's login limiter. Those bound a signed-in member, not an outsider: an unauthenticated flood of the sign-in callback, the pages, or the key-set refetch path reaches the one process that is also serving the take, and a take has no retry budget. "A host nobody is told about" is not a control (scanners find public load balancers within hours). Likelihood stays High (only an address is needed), impact is the take (Medium), and the row is not re-rated.
- Fix: re-rate platform TH14 for the demo (High/Medium, residual Medium as written) and add the cheap control one operator and one browser make possible: a security-group or listener rule admitting only the recording machine's and the canary runner's addresses during the rehearsal period, opened for the audience's reading of the packet never (the packet is a file). Record it as M31 prevent; M32 detect: ALB 4xx and connection counts on the take's dashboard, a spike stops the take. If the allowlist is refused, add a per-client limiter at P2 (token bucket by address) before the session adapter and say that sign-in and the callback are behind it.

### F5. The existing-control column cites the platform's proposed mitigations as controls built today

- Item: R6 (existing controls are separate from new mitigations); guide 10.4 ("an existing control is one built and running today ... a deferred mitigation never lowers residual risk").
- Location: 10.4 TH3 ("platform M6, M7"), TH4 ("platform M1"), TH5 ("platform M3, M14"), TH7 ("platform M22"); appendix D TH8 ("platform M40's locks"), TH9 ("platform M24"), TH10 ("platform M8, M9"), TH11 ("platform M26"), TH12 ("platform M30"); 10.2 entry table ("the session cookie (platform M45)").
- Severity: major.
- Problem: the platform record says every one of its mitigations is [proposed]. The demo's likelihood and residual ratings are "rated on platform 10.4's scale", which rates likelihood against [built] controls; yet nine demo rows list platform M-ids in the existing-control column beside genuinely built items (fAuth's PKCE page, S3 versions, RDS snapshots, the renderer). The demo build is where those platform mitigations are first built, so their status in this record is "proposed, built by the spike or the first build", not existing. A reader cannot tell, row by row, whether the rating holds today or after the build, and the guide's rule that a deferred mitigation lowers nothing cannot be applied.
- Fix: move every platform M-id out of the existing-control column into the mitigation column as "platform M1, built by this record's build, status proposed", keep only [built] items as existing controls, and add one sentence under the 10.4 intro: "The platform's mitigations this build implements are proposed here too; the spike (R11) and the per-build tests are their verification." Where that leaves a High-impact row with no existing control, say "none" and let the residual stand on the proposed mitigation as the platform does.

### F6. One task role connects as all five database roles, so "the platform's credential boundaries survive" is stronger than the mechanism

- Item: guide 10.1 (SA1 and compromise of one is compromise of all), 10.2 (invariants with their enforcers; the trusted computing base kept small), R21.
- Location: 5.2 ("each role keeps its database role, so the platform's credential boundaries survive (D1; platform D11)"); 10.1 SA1 ("the five database roles are the only separation inside the process"); 10.5 ("one ECS task role allowed `rds-db:connect` as the five roles, each with its own pool"); 6.1 ("a page handler's bug has the platform's reach"); 6.2 ("12.2's test").
- Severity: major.
- Problem: on the platform each database role is bound to its own ECS task role (TL7: "four ECS task roles"), so a compromised workspace task cannot obtain a `control` token. In the demo one task role may mint a 15-minute IAM token for any of the five roles, `reset` included, and holds both organizations' KMS contexts, the provider key and the client secret. The separation that remains is which pool a module uses: a compile-time and test-time property, not a credential. That is an honest reduction under AR2, but 5.2 and 10.1 present it as the platform's credential boundary surviving, and 6.1's "a page handler's bug has the platform's reach" is true only for logic bugs; for anything that runs code, the reach is every role and every secret. The trusted computing base is then the whole binary, which 10.2 does not say.
- Fix: reword 5.2 and SA1: "the five database roles separate modules, not credentials: one task role may connect as any of them, so the walls hold against a logic bug and nothing else (AR2)". In 10.2 say the trusted computing base in the demo is the one binary plus P11's guard and P10's filter. Keep 12.2's test but name what it tests (a module cannot write a publication through its own pool) and what it cannot (code with the task role). Add the cheap narrowing that one role allows: per-database-role passwords in Secrets Manager with resource policies readable only by the process's start-up code, each pool opened once at start and the secret handles dropped, so a later compromise must find the open pools rather than mint tokens; and block the credentials endpoint from the parser child (F2). If refused, record the choice under AR2 with the reason.

### F7. Role `reset` writes publications, records and memberships, which contradicts SI3 as stated and widens the trusted computing base

- Item: guide 10.2 (invariants and their enforcers), R21 (security invariants with their enforcing components), R6.
- Location: 6.7 P11 ("inserts the state's rows"); section 8 ("the reset deletes ... definitions, publications, saved work, records, jobs, idempotency outcomes" and restores the state); 10.2 ("The platform's SI1 to SI6 hold with the same enforcers ... role `reset` without a grant on audit (SI5)"; "The trusted computing base adds P11's guard and P10's filter"); 10.4 TH2; AG5 ("Publish without Sarah: TA3, TA11").
- Severity: major.
- Problem: platform SI3 says members see only publications `control` wrote under policy over an approved digest, and a record changes only by a declared command once per key. After a reset to `s4`, members see publication 1 as inserted by role `reset` from a manifest, and records as inserted by `reset`, outside the SERIALIZABLE publisher and the record command. The invariant's enforcer set therefore gains P11, the fixture-state digest (M3) and whoever can write the operator prefix and the pin file, and AG5 gains TA12 (alter `s4` to carry a publication Sarah never approved; M4's diff catches "publication 1 only", not a changed publication 1 unless it diffs digests). Row-level security also does not bound `reset`: it reads and writes both organizations' rows across B7, the one credential that does, and any code in the process can connect as it (F6). None of this is said where SI3 and the trusted computing base are stated.
- Fix: amend SI3 in 10.2: "...or a fixture state P9 captured from a clean take, restored whole by P11 under M3's digest, between takes only". Add P11, role `reset`'s grants and M3 to SI3's enforcers and to the trusted computing base. Make M4's diff compare each restored publication's digest and approval row with the capture's, not only the count. Grant `reset` no SELECT it does not need (it restores from the manifest, it need not read the other organization's live rows) and state its grants in 10.5 beside the other four. Add TA12's path to AG5 and cross-reference TH2.

### F8. The canary suite proves nothing without a positive control and a stated moment

- Item: guide 10.7 ("Can the agreed upon mitigations be tested?"), R21 (mitigations as SR-n with verification).
- Location: SR2 ("The canary suite MUST run per build and per take, every replay 404 ... Test: no non-404"); 10.4 TH4 M8 ("the canary suite per build and before every take, every replay 404 before data"); section 9 ("404: denied, foreign and absent are one answer"); section 8 (the reset deletes rows and outcomes); 7.2 step 8.
- Severity: major.
- Problem: by the platform's own rule a 404 means denied, foreign or absent alike. The suite is specified to run "before every take", when the state is `s0` or `s2`: before act 1 there is no work id, no EXP-002, no `ik-1`; after a reset the outcomes and the rows the last take's identifiers named are gone. A suite that replays identifiers that do not exist returns 404 for absence and SR2 passes with nothing proven. 7.2 step 8 does run the canary after act 6 on live identifiers, and that is the run that counts, but SR2, M8 and the glossary describe the pre-take run as the proof. The test is also unfalsifiable as written: no clause asserts that the same identifiers answered 200 to their owner in the same take.
- Fix: specify the suite as a pair per call: Maya's (or Jordan's, or Sarah's) session resolves the identifier with 200 and its receipt or outcome, then the canary's session replays the identical request and gets 404 before P4; both responses, the same take id and the same build digest are filed in the packet. Run it at the end of every take on that take's identifiers (the pre-take run keeps only the sessionless and forged-principal replays, which need no identifiers). Reword SR2: "every replay 404 against an identifier the owner resolved with 200 in the same take". Add the per-route coverage list (every `/v1/` operation and page) as the suite's manifest so "every call" is checkable.

### F9. Diagram 03 does not carry the demo's own additions, so TH1, TH2, TH5 and TH8 trace to elements that are not on any demo DFD; P10's placement contradicts the text

- Item: R16 (the threat table traces to data-flow element IDs; every container appears in a data flow); guide 10.2 (boundaries keyed to the DFDs), 10.4 (per boundary crossing).
- Location: section 7 opening ("P9 ... P10 ... and P11 the reset are the operator's tools on the host"; "this record's F1 to F13 are on diagram 03"); 10.2 entry table ("`POST /v1/operator/reset` ... F3 to P11"); 10.4 TH1 (P11, F3), TH2 (DS2, P9, P11), TH5 (EE2, P9, P11, F13); appendix D TH8 (P11, F7); 6.7 ("The provisioning script (P9) runs from the operator's shell (EE2)"); 10.2 TL10 ("the operator's shell (EE2), running P9 and P10"); diagram 03 on page 20.
- Severity: major.
- Problem: the demo's threat model is, by design, the platform's plus what the operator's tools add. Those tools are P9, P11, the fixture states in DS2 and the operator's shell EE2, and none of them is drawn: diagram 03 has no P9, no P11, no EE2, no DS2 and no arrow for the reset (operator to P1 to P2 to P11 to DS1 and DS2) or for the fixture capture (P9 to DS2). "F3 to P11" names an arrow that ends at P2 on the diagram. So the rows that are the demo's reason for having a threat model cite elements with no boundary crossing to anchor to. Separately, the diagram puts P10 inside B2 ("the host's network ... the operator's script") while 6.7 and TL10 put P9 and P10 on the operator's shell with the keys; the two placements have different consequences for where the HAR, the packet and the keys sit (F10) and for what F12 crosses. B7, which "rides every arrow" and is TH4's crossing, is not drawn either, though it is the boundary the whole demo exists to prove.
- Fix: add to diagram 03 (or a small diagram 04 for the operator's path): EE2 the operator's shell outside B2, P9 and P10 on it, P11 inside the process, DS2 with the operator prefix, and the arrows F14 operator session to P2, F15 P2 to P11, F16 P11 to DS1 (delete and insert as `reset`), F17 P11 to DS2 (manifest and objects), F18 P9 to DS2 (fixture capture), F19 P9 to EE3 (fAuth management with bearer keys), F20 P9 to P2 (administration group as TL4). Draw B7 on DS1 and DS2 as the platform does. Then re-key TH1, TH2, TH5, TH8 and the entry table to those arrows and name the boundary each crosses. Decide where P10 runs and draw it there once.

### F10. The operator's machine is one asset holding every key, every actor's session, the raw network log and the packet, and it is on camera

- Item: guide 10.2 (assets, blast radius), 10.3, 10.4 (the threats a professional would find).
- Location: 10.1 SA4; 10.2 TL10, TL11, AS11, AS12; 10.4 TH5 ("TA1 with the laptop"), TH6; 6.7 P10.
- Severity: major.
- Problem: the shell's keychain (fAuth's platform key, the management keys, the owner credential, the client secret, role `reset`'s credential), the recording Chrome with Maya's, Sarah's and Jordan's sessions, the exported HAR before P10 filters it, the pin file M3 relies on, and the packet all live on one laptop that is screen-recorded and, in a live rehearsal, possibly screen-shared. The record touches this in TH5 (a stolen laptop) and TH6 (what the camera sees) but no row rates the machine itself: malware, a browser extension in the recording profile, a synced Chrome profile pushing the sessions to another device, or a screen-share leaking the keychain prompt. A raw HAR export of a signed-in browser carries every `Cookie` header and the callback URL's `code` and `state` in its URL fields; M11 strips headers "before it leaves the profile" but nothing says how, and M12's scan covers the packet, not the raw export that stays on disk.
- Fix: add a row: "Whoever controls the operator's machine (TA1 by malware or theft, TA9) can take every key, every actor's session and the unfiltered network log at once, which leads to any principal in either organization and every operator power, negatively impacting AS1, AS11, AS12"; Medium likelihood (a developer laptop with many tools and extensions), High impact. Prevent: a fresh Chrome profile per actor with sync and extensions off, destroyed after the last take; the keychain entries behind the OS prompt with a hardware-backed key where the OS allows; the platform key deleted from the keychain the moment the tenants exist (it is needed once) and reissued if ever needed; HAR exported through Chrome's sanitised export, the raw file hashed, filtered and deleted in one P10 step. Detect: fAuth's user list and session rows compared with the manifest before every take (M10 already), "sign out everywhere" after every take with the epochs bumped. Residual Medium under AR3.

### F11. The hand-inserted `oauth_clients` row means the operator holds fAuth's database, an unlisted credential with the widest reach

- Item: guide 10.2 (assets), 10.6 (per secret its lifetime, rotation trigger, emergency path); R21.
- Location: 6.7 P9 ("the platform's `oauth_clients` row by hand, fAuth having no API for it"); 10.2 AS12 (five keys listed, no fAuth database access); 10.6 per-secret paragraph; AR4.
- Severity: major.
- Problem: "by hand" means the operator writes a row into fAuth's database, so the operator holds a fAuth database credential or a shell on fAuth's host. That credential is not in AS12, not in the rotation paragraph, and not in TH5's prerequisite list, yet it reaches more than any listed key: with fAuth's database one can add users to any tenant, change a client's redirect URI (TH3's attack from the inside), or, at `b750ca53` where the signing key is on the same deployment (platform AR4), mint any principal. The record carries platform AR4 as held but does not connect it to this access.
- Fix: list the fAuth database access in AS12 with its lifetime (the one insert), rotation (the fAuth database password rotated after the row exists; the operator keeps no standing access) and emergency path (fAuth redeployed at the pin, the row re-inserted); add it to TH5's prerequisites and TH3's TA11 path; and cite platform AR4 in AR3's reason. The cheaper structural fix: ship the row as a fAuth migration or seed file at the pin so no one types into fAuth's database, which also makes M6's dry sign-in a check of a file rather than a memory.

### F12. The packet's integrity rests on the person who assembles it; the audience cannot tell a packet from a story

- Item: guide 10.4 (AG "make a number lie", "pass a simulated step as working" are the demo's own High-impact clause), 10.6 (security-event table, tamper protection), 10.7.
- Location: 10.3 AG3, AG4; 10.4 TH6, TH14; 6.7 P10; 10.2 SI8 ("enforced by P10's filter"); 10.6 data dictionary row 3 ("the packet ... M11's filter; insert-only, chained, anchored; a second viewer").
- Severity: major.
- Problem: SI8 says the packet holds only what the browser received and the operator views returned, enforced by P10's filter, but P10 is a script the operator runs on the operator's machine on files the operator exported; the audience (TA13) receives the output. TH14 catches a deploy between acts through the image digest and the deploy log, both read by the operator. Nothing binds the packet to the host's audit chain: an edited receipt, a replaced 404 or a relabelled seeded step is undetectable from the packet alone, and the row 3 claim "insert-only, chained, anchored" is true of the audit table, not of the packet. For a record whose "claim is the product" (10.4 intro), the integrity of the evidence is its top asset and has no row.
- Fix: add a row: "The operator (TA9) editing the packet after assembly, or P10 assembling from the wrong take, can present evidence the host never produced, which leads to a seeded or altered step passing as working, negatively impacting AS10"; Low likelihood (one person, on the record), High impact. Prevent: P10 writes the packet's manifest digest as an audit event through the operator view before delivery, so it lands in the chain under the 15-minute Object Lock anchor; every receipt and outcome in the packet carries the audit sequence it was read at. Detect: the submission names the anchor object and the manifest digest, and a second viewer (already in M12) verifies the chain against the anchor. Residual Low.

### F13. Rows cite elements but not the boundary crossed

- Item: R6 ("cites a boundary crossing"); guide 10.4 ("per boundary crossing rather than per element").
- Location: 10.4 TH1, TH2, TH4, TH5, TH7; appendix D TH8 to TH14 (only TH3 and TH6 name B1).
- Severity: minor.
- Problem: the platform's rows lead with the B-id; the demo's drop it for twelve of fourteen rows, so a reader cannot see which wall each threat crosses, and 10.2's blast-radius paragraph has nothing to key to.
- Fix: add the boundary to each row's first column: TH1 B2 at P2 (and B7 if the flag ever reaches real rows); TH2 B2 at the operator prefix; TH4 B7; TH5 B2 at EE2; TH7 B2 at DS1; TH8 B4; TH9 B1 at DS4; TH10 B6; TH11 B5; TH12 B3; TH13 B4 at the outcome table; TH14 B2 at the task definition.

### F14. The data dictionary omits the decrypted revisions on disk, P4's memory, the raw HAR and the logs

- Item: R21 (data dictionary referenced from the DFDs), guide 10.6.
- Location: 10.6 table; platform 10.6 rows 3 and 4 (DS7, P4's memory) and 7 (audit, logs, traces).
- Severity: minor.
- Problem: on one host both organizations' decrypted Parquet (the platform's DS7) and the result cache sit on the same disk and in the same process under the one task role; the demo's dictionary names DS1, DS2 and AS9 only. The raw network export on the operator's machine (Restricted until filtered), CloudWatch logs and the pre-take assertion log are also missing, and the recording is Internal while it shows Confidential rows (synthetic, so fine, but say the reason).
- Fix: add rows for decrypted revisions and P4's memory (Confidential; the task's ephemeral disk under the platform key; wiped at task stop; one directory an organization), the raw HAR (Restricted; the operator's machine; hashed, filtered, deleted by P10 in one step), and logs (Internal; CloudWatch; allowlisted fields; 90 days), with the F-ids they ride.

### F15. The operator's tenant and the audit reader of operator actions are unstated

- Item: R21 (operator access: breakglass with its audit reader); guide 10.5.
- Location: 4.1 ("Five manual enterprise accounts ... the operator"); 10.5 operator paragraph; 10.4 TH5 M10 ("P10 lists every operator action against the script").
- Severity: minor.
- Problem: the platform signs operators in through Forma's own operator organization; the demo has two tenants and does not say which holds the operator's account. If it is Northstar's, the operator's session is a Northstar member session with an added role, and SA3 ("never a reader of organization data") rests on the permission table alone while the canary suite does not exercise the operator's routes. The reader of the operator's audit is P10, run by the operator; the real reader is the audience through the packet, which should be said (and F12 makes it checkable).
- Fix: state the operator's tenant (recommend a third, operator-only fAuth tenant or the platform's operator organization, so no organization membership carries the role) and add the operator's routes to the canary suite. In 10.5 name the packet's reader as the audit reader for the rehearsal period.

### F16. TA12 and the operator prefix: who reaches it includes the process

- Item: guide 10.3 (actors with capability), 10.2 (assets with the trust levels that reach them).
- Location: 10.3 TA12 ("whoever reaches the operator prefix"); 10.2 AS9 ("P9 writes, P11 reads as `reset`, TL10"); 10.4 TH2 M3 ("the packet's pin file apart from DS2").
- Severity: minor.
- Problem: the one task role reaches the operator prefix (P11 reads it), so TA12 includes any code in the process, not only a person with the operator's AWS access; and the pin file that makes M3 independent of DS2 has no stated location or writer. If it sits on the operator's machine beside the keys, TA12 by the laptop holds both.
- Fix: name the pin file's home (recommend the repository at the tag the build digest came from, so a changed digest is a commit) and its writer; list TL7 among those who reach AS9; add "or the process (AR2)" to TA12.

### F17. SR1's test covers M2 only; SR5's check is not a test of M9

- Item: R21 (mitigations as SR-n with verification on the same line); guide 10.7.
- Location: 4.2 SR1 ("Test: 404 per build"), SR5 ("Check: the packet").
- Severity: minor.
- Problem: SR1 restates M1 to M4, M15, M16 but its test verifies only the unreachability (M2); the digest-matched restore (M3), the manifest diff (M4), the serialisation with in-flight commands (M15) and the row count (M16) have no verification on the line. SR5's "the packet" cannot show that keys stayed in the keychain and out of the repository, history and log (M9).
- Fix: SR1: "Tests: 404 per build; a tampered state 412; P10's diff of each restored state against its boundary; a reset racing a command in S06's two orders". SR5: "Checks: a secret-pattern scan of the repository, the shell history and the rehearsal log before delivery; the packet's operator-action list against the script".

### F18. The risk profile leans on obscurity and understates the exposure window

- Item: guide 10.1 (one-paragraph risk profile).
- Location: 10.1 ("Likelihood is low, thirty minutes on a host nobody is told about").
- Severity: minor.
- Problem: the host is public for the rehearsal period (weeks, with 12-hour sessions and the operator's keys live), not thirty minutes, and not being told about it is not a control.
- Fix: "Likelihood is low for a targeted attack during a rehearsal period of N weeks on a public host reachable only from the allowlisted addresses (F4), and moderate for an opportunistic scan; impact is to the claim ...".

### F19. TH1's M2 test needs a platform-shaped database to run against

- Item: guide 10.7 (continuous checks that can be run).
- Location: 10.4 TH1 M2 ("every build starts the image under the platform's task definition and asserts 404 and no `reset` role").
- Severity: minor.
- Problem: there is no platform deployment yet; the assertion needs a database at the platform's migrations only, with no P9 run, or it tests the demo's database and passes for the wrong reason.
- Fix: say the test provisions a throwaway database from migrations alone, starts the image with the platform's task definition, asserts the route is 404 and `pg_roles` lacks `reset`, then with the flag set asserts the same.

### F20. SI7's enforcer should name the grants, not only the guard

- Item: R21 (invariants with their enforcing components).
- Location: 10.2 SI7 ("never deletes audit or sessions; enforced by P11's startup check and guard").
- Severity: minor.
- Problem: the guard is code in P11; what stops a reset from deleting audit is role `reset` holding no DELETE on the audit table (stated under SI5) and, for sessions, a grant the record does not mention.
- Fix: "enforced by role `reset`'s grants (no DELETE or UPDATE on audit, none on sessions) and P11's startup check and guard", with the grants tested per build beside M2.

### F21. The recording browser's actor switching is unstated

- Item: guide 10.5 (single authentication mechanism; nothing decided on the client).
- Location: 10.1 SA4; 10.2 AS11 ("the actors' three session rows in one Chrome profile"); 7.1 act 4 (Maya submits, Sarah approves).
- Severity: minor.
- Problem: one profile holds one session cookie for the host at a time, so Sarah's approval after Maya's refusal is a sign-out and sign-in or a second profile; which one decides whether a stale Maya cookie can be present when Sarah approves (the approver-not-author CHECK holds either way, but the recording's "session not Maya's on camera" residual in TH3 depends on it).
- Fix: one profile per actor (F10), named in SA4 and AS11; the pre-take dry sign-in run per profile.

### F22. Snapshots outlive the shredding, and the restore drill's environment has no stated owner or end

- Item: guide 10.6 (deletion against immutable stores; rebuild from a known-good state), 10.7 (restore drills).
- Location: section 8 ("one snapshot before each rehearsal"); 10.6 shredding paragraph; 10.4 TH7 M14 ("S06's timed restore into an isolated environment"); platform TH17.
- Severity: minor.
- Problem: RDS snapshots are under the platform key, so the pooled rows the shredding deletes persist in every pre-rehearsal snapshot until the snapshots are deleted; synthetic, so no harm, but the shredding paragraph implies completeness. The restored environment from M14 holds both organizations' data, the flag and role `reset` if restored from the demo's snapshot; nothing says who reaches it or when it is destroyed.
- Fix: add to the shredding paragraph "snapshots deleted after delivery; synthetic rows, so no deadline"; add to M14 "the restored environment is reachable from the operator's address only, has no public listener, and is destroyed within the hour, recorded in the rehearsal log".

### F23. The canary suite's and the pre-take assertion's audit rows have no named writer

- Item: guide 10.6 (security-event table: event, fields, destination, tamper protection).
- Location: 10.6 ("the canary suite's result ... to the log and audit; the pre-take assertion (to the log)").
- Severity: minor.
- Problem: the suite and the assertion are scripts on the operator's shell; audit is insert-only by role and kind (platform M22). Which role inserts the suite's result, and through which route, is unstated, so the row's tamper protection cannot be judged.
- Fix: route both through the operator view as an audit event of kind `rehearsal_check` inserted by `workspace` under the operator's session, carrying build digest, take id and the manifest digest of the suite's request list.

### F24. The one-paragraph "every other platform row holds unchanged" should be a table

- Item: R16 (traceability); guide 10.4.
- Location: 10.4 last paragraph.
- Severity: minor.
- Problem: a reader checking the claim has to hold 29 platform rows against the demo's renames (TH12, TH13, TH15, TH16, TH21 reappear as TH9, TH11, TH12, TH13, TH8) and the four that do not hold (F1 to F4). The sentence also says platform TH10 keeps its walls "without the wall of separate nodes", which is a re-rating in prose.
- Fix: a short appendix table: platform row | demo disposition (unchanged, re-rowed as THn, re-rated with reason, moot with reason). It makes F1 to F4 visible and gives the maintenance trigger something to diff.

## Rows to re-rate

| Row | As written | Re-rate to | Reason |
|---|---|---|---|
| Platform TH8 (forwarded principal) | unchanged, Low/High, residual Medium (AR2) | re-row as demo TH15; Low/High; residual Medium; existing control none, mitigation changed | M16 is not minted; module privacy and the callee's re-read are the control (F1) |
| Platform TH4 / demo TH10 (parser) | TH10 DoS only, High/Medium, residual Low | TH10 as written plus a new EoP row, Low/High, residual Medium (AR2) | an escape lands in the process holding every role and secret; child versus in-process undecided (F2) |
| Platform TH11 (audit rewrite by owner) | unchanged, Low/High, residual Low (AR9) | Medium/High, residual Medium (AR1, AR3) | the owner credential is a standing key in the operator's keychain; no breakglass, no outside reader (F3) |
| Platform TH14 (flood) | unchanged, High/Medium, residual Low | High/Medium, residual Medium until an address allowlist or a P2 limiter exists | the edge limiter was a WAF rule and the WAF is reduced away (F4) |
| Demo TH5 (operator) | Medium/High, residual Medium | keep the rating; widen the statement to AS7 and the fAuth database access | F3, F11 |
| Demo TH2 (fixture state) | Low/High, residual Low | keep; add TA12 includes the process; M4 diffs digests | F7, F16 |
| Demo TH4 (canary) | High/High, residual Low | keep the rating; residual Low only once the suite has its positive control and runs on live identifiers | F8 |
| Demo TH6 (packet leak) | Medium/High, residual Low | keep; add the raw HAR to the prevent | F10 |
| Platform TH17 (deleted data in backups) | held | moot for synthetic rows, say so, with the snapshot deletion | F22 |
| Platform TH27 (compulsion) | removed with TA10 | agreed | nothing synthetic is compellable |

## Threats a professional would add

1. The operator's machine as one asset: keys, three sessions, the raw HAR, the pin file and the packet on a screen-recorded laptop (F10). Medium/High.
2. A parser escape inside the one process reaching every role, both data keys, the provider key and the client secret through the one task role (F2). Low/High.
3. The one task role as a universal credential: any code in the process can mint a token for any of the five database roles, `reset` included, and reach the task's credentials endpoint (F6). Accept under AR2 but say it; block the endpoint from the child.
4. Role `reset` as a cross-organization writer of publications and records outside SI3's enforcers; a fixture state as a publication path for AG5 (F7). Low/High.
5. The canary suite passing vacuously on absent identifiers after a reset or before act 1 (F8). This is a threat to the proof, not to the data: Medium likelihood (it is the default behaviour of the suite as specified), High impact by the demo's own clause.
6. An unauthenticated flood of the public host during a take, with no edge limiter (F4). High/Medium.
7. The operator rewriting audit with the standing owner credential, or the packet after assembly (F3, F12). Medium/High and Low/High.
8. The fAuth database access that the hand-inserted client row implies, reaching any tenant's users and the redirect URI (F11). Low/High.
9. The restored environment of the restore drill as a second copy of both organizations with the reset enabled (F22). Low/Medium.

## What holds

Credit where it is due, so the fixes above are read in proportion. The reset is designed carefully: flag and role both required, 404 without the flag, absent without the role, 412 on a digest mismatch, the synthetic label checked, an idempotency key in the operator's scope, membership rows taken `FOR UPDATE` to serialise with commands, audit and sessions kept, the epoch bumped so tabs re-authorize, and a per-build test that the platform's deployment cannot reach it (TH1, TH8, SI7, AR5). The evidence recorder's filter and its refusal of any capture with a credential header, the cut at the callback, the no-tools-on-camera rule and the packet scan are the right shape (TH6, M11, M12). The two data keys and the organization id in every key from row one (D2) keep the platform's strongest wall on one instance. The ratings scale is inherited with one honest addition (a seeded step passing as working is High impact). Fail-closed is stated per control. Every accepted risk has a reason and an owner. SR1 to SR6 cover M1 to M28 without a gap in the mapping. The attack tree for AG1 names the branch the platform lacks (through the operator), which is the right instinct; the findings above are mostly that branch written out.

## Verdict on the threat model

Sound in structure and honest in most of its reductions, not yet true in four of its "unchanged" claims and two of its central sentences. On one process, one host and one operator, four platform rows change in substance (TH8 the service token, TH4 the sandbox's blast radius, TH11 the owner credential without breakglass, TH14 the edge limiter) and the record carries them as unchanged; the claim that the platform's credential boundaries survive the collapse overstates what one task role allows; role `reset` is a second writer of publications and records that SI3 and the trusted-computing-base statement do not admit; and the canary suite, the proof N7 and G5 rest on, can pass as specified against identifiers that do not exist. None of these needs a new mechanism beyond a positive control in the suite, an address allowlist and a decision on the parser's process model; the rest is writing the rows the demo already implies. With F1 to F12 fixed, the model would meet R6, R16 and R21 and guide item 10 for an L2 record; as it stands it meets the form and misses the substance at the four places where the demo differs most from the platform.
