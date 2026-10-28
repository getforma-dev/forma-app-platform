# Reference cross-check: forma-generative-app-platform-v2

The finished document checked against the research it was built from, by one fresh agent per
slice (`reference/PROCESS.md` section 6, gate item M26): the guide bullet by bullet, every number
recomputed, the pattern catalogue, the reader's edition and the notes against the record, the
teaching layer and its reading list. The reports are in `output/xref/`; this ledger is the lead's
triage of every finding in them. `npm run xref -- forma-generative-app-platform-v2 --ledger` adds a `pending` row for
each finding not yet here.

Record: commit a511eda, checked on 2026-10-23. Slices: guide, numbers, patterns.

## Findings

| Finding | Severity | Disposition | Where or why |
|---|---|---|---|
| G-1 | blocking | fixed | 11.1's image line shortened; the deployment page holds its caption |
| G-2 | should-fix | fixed | 10.4's intro now states the rule: the rows whose controls this record designs stay; the High-impact rows of appendix D are named |
| G-3 | should-fix | fixed | diagrams 02 and 08 relabelled verb first; diagram 03's component arrows name the call's object and stay |
| G-4 | should-fix | fixed | diagrams 04 to 06 re-gridded and relabelled; checked on the rendered pages |
| G-5 | should-fix | fixed | the stores sit inside B2 on diagrams 04, 05 and 06; the control plane leaves flows 2 and 3, which read publications from DS1 |
| G-6 | should-fix | fixed | the sequence diagram's PostgreSQL box is B2 |
| G-7 | should-fix | fixed | section 1 excepts D15's region per organization |
| G-8 | should-fix | fixed | section 2's constraints name the accepted licences |
| G-9 | should-fix | accepted | the body stays above 40 pages after a tightening pass that kept every fact; the head decides at the summary; GR-1 proposes the recalibration |
| G-10 | minor | fixed | section 1 names A1, A2 and A12 |
| G-11 | minor | accepted | section 2's constraints are listed; per-tenant skew is A15's peak pattern and N1's cells |
| G-12 | minor | fixed | the sequence diagram has a lead-in naming it diagram 07 and a simplification sentence; image captions keep their file numbers |
| G-13 | minor | fixed | p99.83 per operation, as N-5 |
| G-14 | minor | fixed | N5 carries its confirmation; AL11 pages an audit or anchor write failure |
| G-15 | minor | accepted | diagram 02 names key management as left out in its narration; diagrams 03 and 08 draw it |
| G-16 | minor | accepted | the GUIDE's twelve columns are folded into eight for legibility on A4; every column's content stays (response with mitigation; likelihood, impact, status and residual together) |
| GR-1 | should-fix | reference | proposed to the head: lower the L3 multiplier or widen the L3 page range; recorded in STATUS and memory |
| GR-2 | minor | fixed | with G-8 |
| N-1 | blocking | fixed | N1 and 11.2 size 10x at 600 operations/s peak: eight data tasks plus one, four workspace, ten workers; the 10x column recomputed |
| N-2 | should-fix | fixed | with N-1 |
| N-3 | should-fix | fixed | ten worker tasks, 80 GiB, with N-1 |
| N-4 | should-fix | fixed | N3 says marginal for p99.9 |
| N-5 | should-fix | fixed | p99.83 per operation in 4.2 and 5.2 |
| N-6 | minor | accepted | rounding between a 21- and a 21.7-day month |
| N-7 | minor | fixed | CloudFront at the HTTPS rate inside the free allowance, $0 at the design point |
| N-8 | minor | fixed | 17 keys in the KMS line |
| N-9 | minor | fixed | 60 requests/s at peak |
| N-10 | minor | fixed | 11.2 states the 730-hour month and 22 business days |
| N-11 | minor | fixed | 11.2 derives the provider counts from one generation and six explanations an organization a business day |
| N-12 | minor | fixed | $0.337 per hour, as the claims check found |
| P-1 | should-fix | accepted | AR1 names it: operator pages share the member plane under the operator role, one founder |
| P-2 | should-fix | fixed | 6.4: the claim sequence fences the result write |
| P-3 | should-fix | fixed | 6.4 says why PostgreSQL is the queue |
| P-4 | should-fix | fixed | section 9 argues 409 over 422 |
| P-5 | should-fix | fixed | 6.3: ISO 4217 code and minor-unit exponent |
| P-6 | should-fix | fixed | DS7 named in section 7, 8 and 10.6; the cold fetch's assumed 200 ms in 6.3 |
| P-7 | should-fix | fixed | section 8: FORCE ROW LEVEL SECURITY, NOLOGIN owner, the per-transaction setting, tests as each role |
| P-8 | should-fix | fixed | 6.7 names connect and request timeouts per dependency |
| P-9 | should-fix | fixed | 6.4 classes provider errors by code with the no-retry rule |
| P-10 | should-fix | fixed | 6.3: a 2 GiB temp directory, process memory watched, the deadline interrupt tested |
| P-11 | minor | accepted | three writers under their roles' prefix policies: workers for revisions and index files, the data service for exports |
| P-12 | minor | fixed | 6.5: one validation definition |
| P-13 | minor | accepted | the 5 s poll is one epoch read; push stays a later refinement |
| P-14 | minor | fixed | AL12 and AL13 |
| P-15 | minor | fixed | 11.1 names the probes and the deregistration rule |
| P-16 | minor | fixed | 11.4's drills include the edge failover |
| P-17 | minor | fixed | section 9: the export writes its file first, then streams |
| P-18 | minor | fixed | section 9: low-entropy keys refused; the island retries by key after a failover |
| PR-1 | minor | reference | PATTERNS 4.4 gains the lock-serialised authority-row form of fencing |
| E-1 | should-fix | fixed | RK9 says six paging alerts; the edition's chapter 6 and the notes' question 27 follow |
| E-2 | should-fix | fixed | the stale gap lines removed from the notes |
| E-3 | should-fix | fixed | the filter's `applies` list no longer names the removed `rate` view, in 6.2 and the edition |
| E-4 | should-fix | accepted | diagram 06 prints inline at the same size in the record; the prose beside it carries the arrow |
| E-5 | minor | accepted | the notes are the author's and may cite the demo record |
| E-6 | minor | accepted | the edition's note names the doors a reader will look up |
| E-7 | minor | accepted | the edition's chapter 2 uses epoch once before chapter 5 defines the three; the summary defines the word |
| E-8 | minor | accepted | the summary runs three-quarters of a page at the edition's font |
| E-9 | minor | accepted | landscape diagram pages leave their lead-in pages short |
| E-10 | minor | accepted | a label touches the control plane's box on diagram 02 at page size; the arrow stays readable |
| E-11 | minor | accepted | the edition gives the 5 s figure; the record's row defines it as the poll plus one request |
| E-12 | minor | fixed | D4's record says aggregates computed at publication, as 6.3 does |
| E-13 | minor | accepted | R14 was fixed in 3.1 at the gate; section 1 keeps its five sentences |
| T-1 | should-fix | fixed | the 11.3 note files the 5 s bound under liveness as the record does |
| T-2 | should-fix | fixed | the 11.2 and 12.1 notes halve the data service's $340 share, not the Fargate line |
| T-3 | should-fix | fixed | note 1 says which identifiers hover and that F and B do not |
| T-4 | should-fix | fixed | the note 8 names the owner exemption FORCE closes and the superuser one no setting closes |
| T-5 | should-fix | fixed | the glossary says all four I gaps are closed |
| T-6 | minor | fixed | the ten permissions are what the trust-level table grants |
| T-7 | minor | accepted | the entry-point table's row count is the record's; the note's phrasing stands |
| T-8 | minor | fixed | the invented history of SR13's numbering removed |
| T-9 | minor | accepted | appendix C's one row holds both linked-in dependencies; the note's wording is close enough for a learner |
| TR-1 | minor | reference | TEACHING.md quotes the NCSC principle as the page words it |
| TR-2 | minor | reference | TEACHING.md says seek method, keyset pagination |
| TR-3 | minor | reference | TEACHING.md names the column as the GUIDE does |
**Disposition** is what the lead did, and the gate holds the ledger to it:

- `fixed`: the record, an edition, the notes, a diagram or the annotations changed; say where.
- `reference`: the finding is about the reference, and the reference is corrected in its own
  `reference:` commit; name the file. Findings numbered with an R after the slice's letter
  (GR-1, PR-1, TR-1) and every S finding are about the reference.
- `accepted`: true, and left as it is; say why. A blocking finding is accepted only by the head,
  and the reason names them.
- `disputed`: the finding is wrong; say why, with the evidence.
- `pending`: not decided yet; the gate fails until none is left.

## Notes

Three findings changed the record: N-1 resized 10x from ten times the average to ten times the peak (eight data tasks, the $4,500 column); the numbers slice's p99.83 and the guide slice's read-path and boundary findings joined the reviewers' in moving the control plane off flows 2 and 3; the patterns slice closed the lease fencing, the RLS bypasses, the timeouts and the provider error classes. Across slices the pattern was the same: the design was sound at the design point and loose at 10x and in the mechanisms a builder would have to invent. For the reference: GR-1, the L3 word ceilings exceed the L3 page range by a fifth, proposed to the head; PR-1 added the lock-serialised authority-row form of fencing to PATTERNS 4.4.
