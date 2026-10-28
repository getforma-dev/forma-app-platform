# Vendor claims: forma-northstar-demo

Record: `tasks/forma-northstar-demo/design.md`, checked on 2026-10-23.

Region for every AWS price: US East (N. Virginia), us-east-1, the platform record's assumed region (platform A9, 11.2). fAuth read at `b750ca534ae89066714f100eb2b76ee71edf1ab1`; Forma read at tag `v3.0.0` (`2006423b21fe0f8dd5a510bd09bfe3b8748327a5`). AWS list prices whose public pages render their tables in JavaScript were read from the AWS Price List bulk CSV for us-east-1 (the URL on the row). The verifier's report is `output/claims-report.md`; this ledger is the lead's disposition of every row. VC51 was added after phase 8's reader check and verified by the lead against the vendor's protocol definition.

## Claims

| ID | Where | The record says | Documentation | Verdict | Disposition |
|---|---|---|---|---|---|
| VC1 | 11.2 | one task: 2 vCPU at $0.04048 per vCPU-hour, 8 GiB at $0.004445 per GiB-hour | https://aws.amazon.com/fargate/pricing/ | confirmed | holds |
| VC2 | 11.2 | 2 vCPU and 8 GiB | https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task-cpu-memory-error.html | confirmed | holds |
| VC3 | 11.2 | db.t4g.small at $0.032 per hour | https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonRDS/current/us-east-1/index.csv | confirmed | holds |
| VC4 | 11.2 | 20 GiB gp3 at $0.115 per GiB-month | https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonRDS/current/us-east-1/index.csv | confirmed | holds |
| VC5 | 11.2 | snapshots free up to the 20 GiB provisioned | https://aws.amazon.com/rds/faqs/ | partly | fixed |
| VC6 | 11.2 | $0.0225 per hour, one capacity unit at $0.008 per hour | https://aws.amazon.com/elasticloadbalancing/pricing/ | confirmed | holds |
| VC7 | 11.2 | under 1 GiB at $0.023 per GiB-month | https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonS3/current/us-east-1/index.csv | confirmed | holds |
| VC8 | 11.2 | two keys at $1 | https://aws.amazon.com/kms/pricing/ | confirmed | holds |
| VC9 | 11.2 | 1 GB logs at $0.50, 30 series at $0.30 | https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonCloudWatch/current/us-east-1/index.csv | confirmed | holds |
| VC10 | 11.2 | three public IPv4 addresses, the load balancer's two and the task's, at $0.005 per hour, no NAT | https://aws.amazon.com/vpc/pricing/ ; https://aws.amazon.com/elasticloadbalancing/pricing/ | partly | fixed |
| VC11 | 11.2 | four secrets at $0.40 | https://aws.amazon.com/secrets-manager/pricing/ | confirmed | holds |
| VC12 | 11.2 | 10 GB out inside the free 100 GB | https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AWSDataTransfer/current/us-east-1/index.csv ; https://aws.amazon.com/ec2/pricing/on-demand/ | contradicted | fixed |
| VC13 | 11.2 | 8,000 in and 3,000 out at $4 and $20 per million | https://platform.claude.com/docs/en/about-claude/pricing | confirmed | holds |
| VC14 | 11.1 | no NAT gateways, egress by the task's public address | https://docs.aws.amazon.com/AmazonECS/latest/developerguide/fargate-task-networking.html | confirmed | holds |
| VC15 | 11.1 | no CloudFront or WAF, the load balancer ends TLS (P1) | https://docs.aws.amazon.com/elasticloadbalancing/latest/application/create-https-listener.html | confirmed | holds |
| VC16 | 11.1 | the load balancer ends TLS (P1) in two subnets of two zones | https://docs.aws.amazon.com/elasticloadbalancing/latest/application/application-load-balancers.html#subnets-load-balancer | confirmed | fixed |
| VC17 | 11.2 | db.t4g.small at $0.032 per hour, its 2 GiB holding IAM authentication's 300 to 1,000 MiB | https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.html | partly | fixed |
| VC18 | 13.1 | ECS replacement unmeasured against 30 s (D10, AR2) | https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task-lifecycle-explanation.html | undocumented | assumed RK8 |
| VC19 | 11.4 | `aws ecs describe-tasks` | https://docs.aws.amazon.com/cli/latest/reference/ecs/describe-tasks.html | confirmed | holds |
| VC20 | 11.4 | `aws rds describe-events` | https://docs.aws.amazon.com/cli/latest/reference/rds/describe-events.html | confirmed | holds |
| VC21 | 11.1 | Snapshot DS1 once, opening the rehearsal (section 8); a retake reuses it | https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_CreateSnapshot.html | confirmed | holds |
| VC22 | 8 | DS1 PostgreSQL, one RDS instance, no standby [reduced: the platform's Multi-AZ] | https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Concepts.MultiAZSingleStandby.html | confirmed | holds |
| VC23 | 8 | DS2 one versioned S3 bucket, a prefix per organization | https://docs.aws.amazon.com/AmazonS3/latest/userguide/Versioning.html | confirmed | holds |
| VC24 | 10.4 | the chain head anchored to Object Lock every 15 minutes | https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-lock.html | confirmed | holds |
| VC25 | 10.6 | DS2 under two data keys; AS9 under the platform's key | https://docs.aws.amazon.com/AmazonS3/latest/userguide/UsingKMSEncryption.html | confirmed | holds |
| VC26 | 10.6 | both data keys scheduled for deletion after the platform's 24-hour hold (platform TH28), unusable at once, gone after the 7-day minimum | https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html | partly | fixed |
| VC27 | 11.1 | The rehearsal flag is one environment variable in the demo's task definition | https://docs.aws.amazon.com/AmazonECS/latest/developerguide/taskdef-envfiles.html | confirmed | holds |
| VC28 | 6.7 | a client secret generated into Secrets Manager and never printed | https://docs.aws.amazon.com/secretsmanager/latest/userguide/intro.html | confirmed | removed |
| VC29 | 6.7 | It reads Chrome's sanitised HAR export, which strips only `Cookie`, `Set-Cookie` and `Authorization` | https://developer.chrome.com/docs/devtools/network/reference | partly | fixed |
| VC30 | 11.1 | Sign in Sarah and Jordan in their own profiles inside the idle hour (the reset keeps sessions) | https://support.google.com/chrome/answer/2364824 | confirmed | holds |
| VC31 | 6.7 | `POST /platform/tenants` for `northstar` and the canary, `PATCH /manage/config` for branding, redirect and invite-only signup, `POST /manage/users` for Maya, Sarah and Jordan | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/mod.rs | confirmed | holds |
| VC32 | 6.7 | the platform's `oauth_clients` row by hand, fAuth having no API for it (A13, D9) | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/db/pg_migrations/001_schema.sql | confirmed | holds |
| VC33 | 6.7 | one redirect URI: a public PKCE client, the code verifier its proof, no client secret | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/token.rs | contradicted | fixed |
| VC34 | 10.5 | by the authorization-code flow with PKCE S256 as a registered client | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/oauth.rs | confirmed | holds |
| VC35 | 5.1 | its EdDSA tokens verified offline against its key set | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/tests/integration.rs | confirmed | holds |
| VC36 | 6.1 | a token without `tenant_id`, optional in fAuth's claims, mints nothing | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/jwt/signing.rs | partly | fixed |
| VC37 | 3.2 | fAuth's passkey button. Server-ready, unwired at `b750ca53` | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/hosted_login.rs | confirmed | holds |
| VC38 | 2 | at `b750ca53`: 44 smoke tests pass, the `northstar` tenant is provisioned | https://github.com/getforma-dev/gatewasm/tree/b750ca534ae89066714f100eb2b76ee71edf1ab1/admin/e2e | confirmed | holds |
| VC39 | 11.4 | the raised burst (FORMA-247); sign-in inside the idle hour | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/middleware/rate_limit.rs | confirmed | holds |
| VC40 | 10.7 | fAuth has no federation for a script and no client API at `b750ca53` | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/token.rs | confirmed | holds |
| VC41 | 10.6 | the tenants soft-deleted at fAuth (a hard purge is a FORMA-247 item); sessions signed out | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/src/handlers/management.rs | partly | fixed |
| VC42 | 10.4 | fAuth's invite-only tenants [built] | https://github.com/getforma-dev/gatewasm/blob/b750ca534ae89066714f100eb2b76ee71edf1ab1/crates/server/tests/integration.rs | confirmed | holds |
| VC43 | 5 | The **[built]** pins are Forma 3.0.0 `2006423b` | https://github.com/getforma-dev/forma/releases/tag/v3.0.0 | confirmed | holds |
| VC44 | cover, before 1 | The *28,100 B budget* is Forma's gzip ceiling for one route's Forma payload (BUILD-819) | https://github.com/getforma-dev/forma/blob/2006423b21fe0f8dd5a510bd09bfe3b8748327a5/docs/ARCHITECTURE-3.0.md | confirmed | holds |
| VC45 | 13.1 | The spike's unknowns on two-day-old Forma 3.0.0 | https://github.com/getforma-dev/forma/commit/2006423b21fe0f8dd5a510bd09bfe3b8748327a5 | confirmed | holds |
| VC46 | C | the renderer refuses a computed kind [built: BUILD-610, review 4.3] | https://github.com/getforma-dev/forma/tree/2006423b21fe0f8dd5a510bd09bfe3b8748327a5/docs | undocumented | fixed |
| VC47 | 7.2 | Revocation takes the membership row `FOR UPDATE` while the command holds it `FOR SHARE`, so they serialize (F7) | https://www.postgresql.org/docs/current/explicit-locking.html | confirmed | holds |
| VC48 | 8 | READ COMMITTED with explicit locks for commands and adoption, SERIALIZABLE for publication | https://www.postgresql.org/docs/current/transaction-iso.html | confirmed | holds |
| VC49 | 8 | Every table carries `FORCE ROW LEVEL SECURITY` under a `NOLOGIN` owner; no runtime role owns one or bypasses a policy | https://www.postgresql.org/docs/current/ddl-rowsecurity.html | partly | fixed |
| VC50 | 4.2 | an automated WCAG 2.2 AA check on every state of the demo's components | https://www.w3.org/TR/WCAG22/ | confirmed | holds |
| VC51 | A | over the Chrome DevTools Protocol: it brings one actor's profile forward at a time and makes 13.3's simulated faults at the client, a response held late, a response dropped after the server answered | https://chromedevtools.github.io/devtools-protocol/tot/Fetch/ ; https://github.com/ChromeDevTools/devtools-protocol/blob/master/pdl/domains/Fetch.pdl | confirmed | holds |

## Notes

- **VC1** "CPU cost: $0.000011244 per vCPU second, memory cost: $0.000001235 per GB per second", US East (N. Virginia); times 3,600, $0.04048 and $0.004446; the record's rounding gives the same $85.
- **VC2** "2048 (2 vCPU): Between 4 GB and 16 GB in 1 GB increments".
- **VC3** "$0.032 per RDS db.t4g.small Single-AZ instance hour (or partial hour) running PostgreSQL".
- **VC4** "$0.115 per GB-month of provisioned GP3 storage running PostgreSQL".
- **VC5** "Free backup storage is provided up to your account's total provisioned database storage across the entire region"; was: "snapshots at $0.095 per GiB-month". One pre-rehearsal snapshot of a 20 GiB instance costs nothing until the chain exceeds 20 GiB.
- **VC6** "hourly charge of $0.0225" and "$0.008 per LCU"; 730 hours give $22.
- **VC7** "$0.023 per GB - first 50 TB / month of storage used".
- **VC8** "Each AWS KMS key that you create in AWS KMS costs $1/month (prorated hourly)".
- **VC9** "$0.50 per GB custom log data ingested" and "$0.30 per metric-month for the first 10,000".
- **VC10** "Hourly charge for In-use Public IPv4 Address $0.005" and "standard public IPv4 address charges for all the addresses you consume with load balancers"; was: "IPv4 at $0.005 per hour, no NAT", one address. An internet-facing load balancer holds one per zone, two, plus the task's.
- **VC11** "$0.40 / secret / month".
- **VC12** "$0.090 per GB - first 10 TB / month data transfer out beyond the global free tier" and "100 GB of free data transfer out to the internet free each month"; was: "10 GB out at $0.085", the second tier's rate. The 10 GB is inside the free allowance: $0.
- **VC13** "Claude Opus 5.5: $4 / MTok ... $20 / MTok". The vendor adds that models from Claude 4.7 on tokenize about 30% more tokens for the same text; N10's meter measures the 8,000 and 3,000 on the model in use, so the estimate is checked at the first rehearsal, not corrected here.
- **VC14** "When using a public subnet, you can assign a public IP address to the task ENI".
- **VC15** "The load balancer uses a server certificate to terminate the front-end connection".
- **VC16** "You must select at least two Availability Zone subnets ... Each subnet must be from a different Availability Zone"; was: "The load balancer's two subnets and the IPv4 charge: vendor claims to verify", an open question in 13.2, now closed into 11.1.
- **VC17** "IAM DB authentication requires compute resources on the database instance. You must have between 300 and 1000 MiB extra memory"; added to the RDS line: the instance's 2 GiB holds it.
- **VC18** AWS documents the task's states (PROVISIONING, PENDING, ACTIVATING, RUNNING) and no duration; the record keeps the replacement unmeasured, RK8 names it and the in-place supervisor is the hedge (11.2).
- **VC19** The output documents `tasks[].containers[].imageDigest`, `lastStatus`, `startedAt`, `pullStartedAt` and `pullStoppedAt`: enough for P10's digest check (M28) and for timing a replacement.
- **VC20** "Returns events related to DB instances ... for the past 14 days".
- **VC21** "Creating this DB snapshot on a Single-AZ DB instance results in a brief I/O suspension"; taken before the take, outside any scene. "Snapshots don't expire."
- **VC22** Multi-AZ "automatically provisions and maintains a synchronous standby replica in a different Availability Zone"; a Single-AZ instance has none.
- **VC23** "By default, S3 Versioning is disabled on buckets, and you must explicitly enable it."
- **VC24** "Object Lock works only in buckets that have S3 Versioning enabled"; the 15-minute cadence is the platform's own (AR9).
- **VC25** "the AWS KMS keys must be in the same Region as the bucket"; the default encryption context is the object ARN, so the prefix is bound to its key per object.
- **VC26** "AWS KMS requires you to set a waiting period of 7 - 30 days" and a key pending deletion "cannot be used in any cryptographic operations"; was: "scheduled for deletion through the platform's 24-hour hold". The keys are unusable at once and gone after the minimum; the 24-hour hold is the platform's own delay before scheduling.
- **VC27** "Environment variables specified in the task definition are readable by all users and roles that are allowed the DescribeTaskDefinition action": a flag, not a secret.
- **VC28** Secrets Manager stores "database credentials, application credentials, OAuth tokens, API keys, and other secrets"; the AWS side held, but there is no secret to store (VC33), so the sentence is gone from 6.7, M5, AS12 and the secrets paragraph.
- **VC29** Chrome's "Export HAR (sanitized)" "excludes sensitive information such as Cookie, Set-Cookie, and Authorization headers", not every header; was: "every header stripped before it leaves the profile (F12)". P10 drops the rest and refuses an export that still carries those three (6.7, 7.2, TH6).
- **VC30** "With profiles, you can keep all your Chrome info separate"; separate sign-ins per profile is the documented use.
- **VC31** `mod.rs` registers `/platform/tenants` (POST), `/manage/config` (PATCH) and `/manage/users` (POST).
- **VC32** `oauth_clients (id, redirect_uri, name, tenant_id)`; the only inserts are the hosted page's own client and onboarding; no route registers a third-party client.
- **VC33** The table has no secret column; the token exchange matches code, client and redirect URI and requires the PKCE verifier ("Missing code_verifier (PKCE required)"); was: "with exactly one redirect URI and a client secret generated into Secrets Manager". One redirect URI held (`redirect_uri text NOT NULL`); the secret did not. FORMA-247 amended.
- **VC34** `oauth.rs`: "Only code_challenge_method=S256 is supported".
- **VC35** `/auth/jwks` serves `"alg": "EdDSA"`, `"crv": "Ed25519"` with a `kid`.
- **VC36** In the `Claims` type `sub` is required and `tenant_id` an optional string; added to 6.1 and 10.5: a token without it mints nothing.
- **VC37** The WebAuthn login routes are registered; the hosted page and the admin login contain no passkey reference.
- **VC38** The suite's log at the pin: "44 passed (30.2s)".
- **VC39** `rate_limit.rs`: "Override burst size with LOGIN_RATE_LIMIT_BURST env var".
- **VC40** `token.rs` matches only `authorization_code` and `refresh_token`; no client-credentials grant exists.
- **VC41** `DELETE /platform/tenants/:id` is "Soft-delete a tenant (set status = 'deleted')"; was: "the tenants deleted at fAuth, sessions signed out". The rows remain; a hard purge is a FORMA-247 item.
- **VC42** `signup_mode` accepts `invite_only`; the test `invite_only_register_requires_invite_and_accepts_valid_invite` asserts `invite_required`.
- **VC43** Tag `v3.0.0` resolves to `2006423b21fe0f8dd5a510bd09bfe3b8748327a5`.
- **VC44** `docs/ARCHITECTURE-3.0.md`: "N2 is 28,100 Forma gzip bytes per route under L-64"; `docs/RELEASING.md` names BUILD-819 as the build's refusal.
- **VC45** The tag's commit is dated 2026-10-20; the record 2026-10-22.
- **VC46** Nothing in the v3.0.0 `docs/` tree says the renderer refuses a computed kind; was: "the renderer refuses a computed kind [built]". The [built] tag now cites the evidence it rests on: BUILD-610 and the platform's native composition review (review 4.3).
- **VC47** "A shared lock blocks other transactions from performing UPDATE, DELETE, SELECT FOR UPDATE".
- **VC48** "Read Committed is the default isolation level in PostgreSQL"; Serializable "emulates serial transaction execution for all committed transactions".
- **VC49** "Table owners normally bypass row security as well, though a table owner can choose to be subject to row security with ALTER TABLE ... FORCE ROW LEVEL SECURITY"; added to section 8: every table under FORCE ROW LEVEL SECURITY, owned by a NOLOGIN role.
- **VC50** "Web Content Accessibility Guidelines (WCAG) 2.2", W3C Recommendation, 12 December 2024; AA is the middle of three conformance levels.
- **VC51** Added after the reader check, verified by the lead in the protocol's own definition: "Response will intercept after the response is received (but before response body is received)"; a paused request waits "until client calls one of failRequest, fulfillRequest or continueRequest"; `failRequest` "Causes the request to fail with specified reason." Holding a paused response delivers it late; failing it drops an answer the server already gave.
