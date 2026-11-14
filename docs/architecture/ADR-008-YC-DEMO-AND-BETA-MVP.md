# ADR-008 — The MVP is the YC demo plus a live invite-first beta

Date: 2026-10-29. Owner: GenUI. Status: founder direction (quoted in the [disposition record](../../evidence/APP-006/founder-disposition-2026-10-29.md)) plus GenUI's interpretation, recorded for Victor's veto. GenUI's always-on hosting figure remains his choice (below); he approved FormaStack's lean fAuth (FORMA-367) with SES SMTP the same day. Victor's words are only the quotes in the record; every ticket placement, gate action and deferral of GenUI's own work below is GenUI's or FormaStack's application of them. This amends the sequencing of [ADR-005](ADR-005-PHASE-1-SEQUENCING.md) and the acceptance ownership of ADR-003's hosted items; it changes no behavior contract in ADR-003, ADR-004, ADR-006 or ADR-007.

## Decision

1. **The MVP is two things on one build.** The Northstar demo of the demo design (six acts, S01–S07, one image digest, the packet) recorded for YC, and the same build deployed at `https://beta.getforma.dev` as an invite-first beta that invited viewers can sign up to and use on synthetic data, labeled beta, with no SLA. Phases 1–4 deliver it; the platform-v2 design remains the code's design and the demo design its reduction.
2. **Ceremony deferred, not waived.** The joint six-hour identity rehearsal, the native Safari and Firefox runs, the deployed negative matrix, the KMS rotation campaign and the Canary-versus-Northstar resource check move to **APP-079** (PH06, consumer of FORMA-365). GenUI's snapshot/restore drill, two-process race coordinator on RDS and drift/replica trials move to **APP-080** (PH05). The APP-080 deferral of GenUI's own RDS rows is GenUI's interpretation of "fail forward", not a decision Victor named. Both tickets block any real organization. A sentence moved unchanged appears verbatim in its new owner with a "(Moved from APP-xxx by ADR-008.)" marker; APP-074's items 1 and 8 were split by browser (the installed-Chrome text edited in APP-076, the native Safari and Firefox text verbatim in APP-079); every split, reworded or new sentence carries no "Moved" marker and is listed in §5.
3. **Beta-critical hosted items stay in Phase 1.** Real invited signup with mailbox verification (ADR-006 routes against FormaStack's mail image, FORMA-366), the admitted query → receipt → command → reopen journey in one browser on the live deployment, canary 404s over public HTTPS, and the migrations, seed, qualification-entry run and HTTP surface on RDS. "Fail forward" means a found bug is fixed and redeployed, not that the item is skipped.
4. **Gates.** FORMA-247 and FORMA-248 are cleared by disposition in the external register with the disposition record as evidence; FORMA-366 (the qualified mail image and controlled-inbox trial) and FORMA-367 (FormaStack's lean always-on fAuth deployment, split out on 2026-10-29) block APP-076, the go-live trial; FORMA-365 (deferred hosted acceptance) blocks APP-058 and APP-079. APP-006 proceeds on its local consumer qualification and the hash-bound merged contract (gatewasm PR18/19 at `dfba787`, PR20 at `1bf5d3b`), with its hosted items moved verbatim to APP-079.
5. **Re-scoped tickets.** APP-078 keeps readiness comparison, migrations, seed, role bindings, the grant/RLS isolation matrix, the HTTPS surface and cleanup; APP-076 becomes the beta go-live trial (installed Chrome, real mail, private-route and canary refusals, migrations applied, rollback as a redeploy, service left running for the beta) and takes APP-074's hosted items (items 6 and 7 verbatim; items 1 and 8 as installed-Chrome edits); APP-077 keeps the operator package and hands the coordinator to APP-080; APP-074 proves the public invite entry, limits, trusted proxy and admission races on the local harness against the test-only synthetic issuer. APP-079 carries native Safari and Firefox, the deployed negatives, the rotation campaign and the canary resource check; APP-080 the recovery drill, pool-lifecycle, two-order concurrency and drift rows on RDS, and APP-076's former RDS race-order item verbatim. Edits that are not verbatim moves:
   - APP-074 items 1 and 8 are split by browser. APP-076 items 3 and 4 carry the installed-Chrome text, labelled "Edited from APP-074", pointing native Safari and Firefox to APP-079 and, in item 4, naming the FORMA-366 mail image and the FORMA-367 deployment with its separately approved hosted-auth allowance instead of FORMA-247. APP-079 carries both items verbatim for native Safari and Firefox.
   - APP-074 gains a trusted-proxy and bounds item citing ADR-007 D7.14, and its §Why and §What changes describe the local harness and the pinned merged PAR contract.
   - APP-076's rollback/cleanup item is rewritten for an always-on service (RUNNING on the recorded digest instead of STOPPED; the ledger reconciled against the figure Victor approves, ADR-003's $100/month cap standing until he does); protected work stays disabled during rollback and its allowance-before-reservation sentence is kept.
   - APP-076's former RDS race-order item moved verbatim to APP-080; APP-076 replaces it with public-API checks of last-slot redemption, identical retry and revocation at delivery. Its integrated-path item says "on the live deployment" instead of "on the managed environment" (the same environment).
   - APP-077's review item is split: the package items stay in APP-077 (limitations "for APP-078"), the coordinator items go to APP-080 (limitations "for the PH05 gate record"); both are labelled "Split from APP-077".
   - APP-078 keeps the administrator-operation limitation rule; APP-080 repeats it for the drift trial, and its drift-target sentence is reworded from APP-078 item 1. APP-078's snapshot item keeps the snapshot's creation and moves the restore and comparison to APP-080, whose copy adds a recorded cutover or forward-fix procedure. APP-078's cleanup item verifies temporary task-definition cleanup and retains and names the pre-migration snapshot (temporary restored instances are deleted under APP-080), and "a partial matrix" became "a partial run of the rows above".
   - APP-079 gains three items with no "Moved" marker: the admitted journey in native Safari and Firefox, the canary organization's denial of a Northstar application resource (from the APP-006 rehearsal plan) and the bounded unknown-kid refetch on the live issuer (from APP-006's hosted-key campaign).
6. **Phase 1 exit criteria** are amended in `Tickets/plan.json` to name these owners; the gate record lists each deferral with its ticket, per QUALITY-GATES.
7. **Repositories.** This repository is public under its all-rights-reserved notice so GitHub Actions run; Victor directed on 2026-10-29 that it returns to private after deployment ([ADR-005](ADR-005-PHASE-1-SEQUENCING.md) §8). The beta deployment is now ongoing, so "after deployment" has no fixed end; Victor decides when. gatewasm is public under BUSL-1.1 (FormaStack, PR 21).

## Victor's choices for the beta (defaults in bold)

- **FormaStack's lean fAuth: approved.** Victor to the FormaStack session on 2026-10-29: "Approved: deploy the lean always-on fAuth (FORMA-367) at about $33/month using SES SMTP. Coordinate the timing with GenUI." GenUI reads "using SES SMTP" as the mail-transport choice below; FormaStack configures it.
- **Hosting budget for GenUI's live beta: pending.** [SYNTHETIC-ENVIRONMENT-COST.md](../planning/SYNTHETIC-ENVIRONMENT-COST.md) prices the always-on synthetic environment (one 2 vCPU/8 GiB Fargate task, RDS, ALB, addresses, keys and a modest usage allowance) at about $150–170/month (priced for Linux/x86 Fargate; the task is ARM64, which is lower, so the figure is a conservative estimate, not a quote) and says the steady-state plan must be requalified before inviting external beta users; FormaStack recomputed a lean always-on fAuth (App Runner 0.25 vCPU, RDS t4g.micro, NAT, IPv4, three KMS keys, secrets, logs, SES at $0.10 per 1,000 mails) at about $31–38/month, quoted as "about $33, up to $38" (FormaStack's figure, not verified by GenUI). The default is **about $185–210/month while the beta is advertised**; the alternative is a scheduled daily service window to stay nearer the $100 cap. ADR-003's $100/month was set for a bounded 48-hour qualification window and the earlier $200 always-on option was not approved, so an explicit new figure is needed. Until Victor approves it, ADR-003's $100/month cap governs every GenUI window.
- **Mail transport (answered for fAuth by the FORMA-367 approval, as GenUI reads it).** **Amazon SES SMTP credentials in the same account with a verified `getforma.dev` sender, configured through fAuth's existing SMTP secret (host/username/password/from_email, port 587 STARTTLS) by FormaStack, DKIM/SPF/DMARC records by GenUI**; FormaStack prefers this because Resend would need new CDK wiring. A new SES account starts in the sandbox and can mail only verified addresses, which covers the controlled-inbox trial but not arbitrary beta invitees; production access is an AWS review of about a day and should be requested as soon as the transport is chosen. Without a transport, invited outsiders cannot verify and the beta cannot admit them.

## Consequences

The execution plan's Track C becomes the beta go-live path: FormaStack deploys fAuth lean under FORMA-367 (about 1–1.5 hours of its work; Victor approved it with SES SMTP on 2026-10-29) and qualifies the mail image under FORMA-366; GenUI deploys the final reviewed build with migrations applied (APP-078), provisions invite codes, and runs APP-076 against it in installed Chrome. The deferred ceremony is recorded in the PH01 gate record as a named limitation, not omitted. Until APP-080 exercises the restore, the pre-migration snapshot is retained but unproven; during the beta the only rollback for a committed migration is a reviewed forward fix. Phase 2 (imports, interpretation, canvas, forms) and Phases 3–4 (authoring, AI, publication, two-app proof) follow for the demo's acts 2–6.

## Victor's decisions of 2026-10-29 (evening)

- **GenUI always-on figure approved:** about $150–170/month (x86-priced; ARM64 is lower) for the advertised beta, on top of FormaStack's lean fAuth (FORMA-367, about $33/month), replacing ADR-003's bounded-window $100/month for that period. The ledger basis is rewritten to this figure before the next reservation.
- **SES sandbox accepted for the MVP:** invitation mail goes to recipients verified in SES ahead of each trial (currently `victor@getforma.dev`), which satisfies FORMA-366's controlled-inbox trial; SES production access remains pending and is not required.
- **Window length:** hosted runs for the MVP (APP-078, APP-076) are kept to minutes and the long campaigns stay in the backlog (APP-079, APP-080), matching the existing split.

## Amendment 2026-11-14: a real live test with 25 real people

Victor decided on 2026-11-14 to run a real live test of the platform with real people, enforced to production standards ([plan](../planning/TRIAL-25-READINESS.md)). This amendment replaces Decision item 1's "synthetic data" for this test only. Everything else in this ADR stands.

- **First cohort.** The 25 adult students of Victor's weekend class (2026-11-29 and 2026-11-30) join **Victor's organization**, today's clean-trial organization `forma-trial`, as members. Victor is its admin.
  - Victor builds apps for the class with the platform.
  - Students use those apps, and can build and share their own apps for anything.
  - The platform stays general: no classroom feature, name or template is added to it. The class is a use case that tests whether the platform can be shaped to it.
- **Access.** Invite-only. The operator issues codes, and Victor delivers them by email. Codes go out on 2026-11-28, after a rehearsal with two or three students on 2026-11-26. Real use continues through 2026-12-12.
- **Enforced before codes go out:**
  - durable, enforced consent to beta notice version 1 (below), stored and audited;
  - real member names that each member sets;
  - an Organization Admin area for the organization's admins (`admin_policy`), with member removal;
  - record lists that keep entries private to their creator and the app's reviewers;
  - reviewer-only commands;
  - sharing with roles and with everyone in the organization;
  - an app-wide daily model-call cap and a spend limit on the provider account;
  - email alarms, one restore drill, and an operations runbook.
- **Accepted risks** (Victor, 2026-11-14):
  - APP-079 (hosted identity acceptance, FORMA-365) and APP-080 (RDS races, drift and restore comparison) are unfinished. The single restore drill covers only part of APP-080.
  - The APP-078 managed isolation verdict is deferred, because there is one real organization.
  - One Fargate task: every deploy is an outage of about four minutes that signs everyone out. No deploys from 2026-11-28 through the class, and later only in announced windows.
  - A single-AZ `t4g.small` database. Logs are kept 30 days.
  - No per-organization quotas beyond the daily model cap and the per-member hourly limits.
  - Container findings carried under a temporary, reviewed disposition.
  - Deletion during the test is limited to removing access and deleting uploaded files. All other data is purged with the organization after the test.
  - The stack keeps its `Environment=synthetic` tag, because an infrastructure change would block promotion. That is a known mismatch.
  - Victor is the only operator, and alarms reach him by email; no one is paged.
  - **One classroom network.** Sign-in limits count per IP address (`app/src/http/limits.rs`), and a classroom usually shares one. Students redeem their codes at home on 2026-11-28, and the per-address limits for invitations and sign-in are raised before the last deploy.
  - **An outage during class.** The class continues without the platform. Victor keeps an offline fallback for attendance and quizzes, and says at the start that this is a beta. During class a broken beta is observed, not repaired.
  - **Not the record of record.** Grades and attendance in the platform are not the official record, and no export exists. Victor copies them out before 2026-12-12; the purge procedure starts with that copy-out, and students are told their apps are deleted by 2026-12-14.
  - **Phones.** Desktop first. Phones serve check-ins and submissions; building apps on a phone isn't supported during the test. Students bring a laptop.
  - **Withdrawal.** Declining the checkbox, or leaving, costs nothing in the class, because the fallback serves them. Victor removes the member in `/admin` at once and deletes their uploads within two days. Their entries in shared apps stay until the purge, as the notice says.
- **End of the test.** The organization's data is purged by **2026-12-14**, by the procedure in the operations runbook (`docs/operations/TRIAL-RUNBOOK.md` §10):
  - **Content deleted:** apps, records, uploads, names, consents and memberships, plus every stored file version.
  - **Kept:** the hash-chained audit log (identifiers, event kinds, times and hashes; no content) and the routing rows for the organization's handle.
  - **Backups:** manual snapshots are deleted at the purge. RDS automated backups lapse seven days later, so the data is fully gone around 2026-12-21.
  - **Sign-in accounts:** the members' sign-in accounts (email and password) live in the fAuth tenant run by FormaStack, outside this database. The purge procedure includes a written request to FormaStack to delete the tenant's users by 2026-12-14, with the confirmation recorded.
  - **Guest shares:** the organization's guest shares and guests' links to them are deleted. Each guest account's share-event records (identifiers, kinds, sequence and hashes; no title, address or content) are kept, so the guest's hash-chained history still verifies. This relies on migration 0024 dropping the key from those records to the share rows. Until that ships, the purge refuses safely if a guest has linked a share.
  - The beta stops about 2026-12-15 unless it is renewed again.

### Beta notice, version 1

Shown on the invitation page; the required checkbox records consent to this version. The text is general, because it is platform text, not text for one cohort. Approved on 2026-11-14 by the Fable advisor acting for Victor at his instruction ("Ask Fable for answers and directions in lieu of me").

> **Before you join this Forma workspace**
>
> Forma (getforma.dev) is in early beta. It may change, break or go offline without notice, and there is no service guarantee. You must be 18 or older to take part.
>
> You are joining an organization's workspace. Its admins can see who has joined, your agreement to this notice, when you were last active, and how much you have created. Apps shared with you may collect what you enter. The app's owner and its reviewers can see what you enter; unless the app keeps entries private, so can the other people it is shared with. Anything you build is private unless you share it. Forma's operators can access stored data to run and support the service.
>
> **What we keep:** your email address for sign-in, the name you choose, what you create and enter, the files you upload, and basic activity logs. Data is stored in the United States.
>
> **AI:** to build apps and answer questions, your prompts, and the files and entries you ask it to work with, are sent to Anthropic, our AI provider. Please don't upload sensitive personal, health or payment information.
>
> **Your data:** you can ask at any time, at feedback@getforma.dev, to remove your access and delete the files you uploaded; entries you made in shared apps stay until the end of the test. This workspace's data is deleted after the test, by December 14, 2026; backup copies expire about a week later. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider; we ask it to delete the account after the test. Copy out anything you want to keep before then.
>
> By checking the box you agree to take part on these terms.
