# APP-008 compatibility and lookup-order review

**Latest verdict: no blocking findings at the final follow-up source below.** The reviewer did not author the schema/runtime implementation. This supplements the [preserved R1–R3 review](independent-review.md); it does not replace its findings, source identities or earlier results. [This review's JSON](independent-compatibility-review.json) binds the exact source and executions. The initial temporary-relation finding and failed reproduction remain below as history.

## Reviewed compatibility changes

The canonical projection now excludes only incoming membership rows whose member is `genui_migration`, parent is one of the six named foundation roles, and flags are exactly `admin=true`, `inherit=false`, `set=false`. The explicit operator owner/helper grants, every other incoming member or flag and all fixed-role outgoing reachability remain compared. The normalization does not change database privileges or learn a baseline from managed state.

Independently ran the **154-case real PostgreSQL16.13 campaign** at runtime `c6bc3ca14dd898597cd78651d7baecf3a79676dd8d8d7d79369b5c6d2732ca09`, query `dd86a4be87895fe4ddb8ccd24e31da50f6705754ffc2dc51709c1b7171f24422` and version2 catalog `51418a12cf45809b2e4a3ea7e9ee6deeed8d2189a4dbc936fe220698ef031954`. All passed, including omitted optional grants at version1/2, refusal of missing explicit owner access, unrecognized incoming members and noncreator grant flags. All nine owned database containers were removed. The [full report](independent-postgres-compatibility.json) also retains the earlier RLS, ACL, FK, replica-mode, history and two-order race checks.

Independently regenerated both catalogs in another fresh owned database: candidate bytes exactly equal the authoritative version1/version2 files, with cleanup confirmed in the [generation receipt](independent-catalog-reproduction-compatibility.json). No expected state was taken from RDS. The **30 runtime tests passed** in 11.599 seconds, retaining the auth-file and default empty-registry lifecycle checks.

Setting the transaction search path before migration identity, advisory-lock and metadata lookup closes the ordinary database-default shadow path. The campaign proves both supported existing versions refuse an unsafe persisted search path without running its functions or repairing the persisted default; a replaced legacy version view is refused before its expression runs. The remaining session-local case below is narrower.

## R4-SCHEMA — Explicitly order the temporary schema after the catalog

**P2**, [runtime.py](../../../app/runtime/runtime.py), catalog-verification line359 and migration-entry line385 at the recorded runtime hash. PostgreSQL implicitly searches the session's temporary schema before relations in `search_path=pg_catalog` when `pg_temp` is not explicitly ordered.

The independent [reproduction](reproduce-temp-catalog-shadow.py) first installed valid version2, then created a temporary `pg_namespace` view over the real catalog with a volatile sequence marker on the operator connection. Both `verify_schema_catalog()` and `apply_migrations()` accepted the unchanged expected catalog while the temporary expression ran **503 times per call**. The [actual observation](independent-temp-catalog-shadow.json) records the source pins, `search_path=pg_catalog` and owned-container cleanup.

This requires a caller-supplied operator connection that already contains temporary relations. Normal `migrate()` opens a fresh connection, and application roles lack TEMP; this result does **not** establish a public application exploit. It does show that the reusable catalog guard's claimed catalog-only lookup is incomplete on an existing session. Explicitly order `pg_catalog, pg_temp` or fully qualify catalog relation references, and retain a positive-control marker test followed by zero-marker verification/migration checks.

Root and the schema author were notified. The repair must retain this failed observation and append a source-bound result rather than overwrite it. The separate Python readiness helper's pre-catalog metadata ordering was also sent to its integration reviewer for assessment; it is not represented here as a completed reproduction or verdict.

Actual current RDS catalog compatibility, IAM/TLS, managed recovery, source/image CI and hosted identity/session/admission remain separate gates. Neither the compatibility normalization nor these local results complete APP-008 or Phase1.

## Final follow-up — lookup repairs verified

**No blocking findings** at runtime `dfef99b5f60fe225b480563a8b54977cfdc45f99887b9a05a9904b4a3f918beb`, qualifier `84ac10d54a201de496f547afe05f181a441b263be1b07063170e0a8b2cae6be0` and query resource `e17ceb5e62dc6f03dfb9dc26f34018d3f2977e1b93ee5910ed16f03364a63963`. The query SQL and version1/version2 catalog values remain unchanged; query metadata now specifies `pg_catalog, pg_temp`. All thirteen reviewed source/resource/ADR inputs remained unchanged through the final independent checks.

R4-SCHEMA is resolved: catalog verification and migration entry explicitly order `pg_temp` after `pg_catalog`. Repeating the independent original reproduction now accepts the valid catalog without evaluating either temporary view expression; both marker flags remain false, as retained in [the repaired reproduction](independent-temp-catalog-shadow-repair.json). Its owned container was removed. The earlier 503-call observations were not overwritten.

The Python readiness helper also performs canonical catalog and effective-origin verification before reading application-owned version metadata. Its connection starts with the fixed lookup path, and its subsequent fixed-role identity queries use qualified system catalog names. This prevents parallel native/Python readiness execution from relying on native failure to protect a separate, already-running helper query.

The final **156-case real PostgreSQL campaign passed**, including the explicit temporary-view positive control and the helper's replaced-version-view refusal. For the helper, the read-write control invokes the actual marker, the read-only control reaches the expression and raises PostgreSQL `25006`, and the repaired read-only helper instead raises `runtime.Refused` before expression evaluation. The original marker remains untouched. This distinguishes catalog rejection from a write merely being blocked by read-only mode. All nine owned campaign containers were removed; see [the final report](independent-postgres-final-lookup.json).

The **31 runtime tests passed** in 11.600 seconds, including refusal before helper metadata or storage activity; [the test output](independent-runtime-final-lookup.txt) is retained. A new independent generator container reproduced both authoritative catalog files byte-for-byte and was removed; see [the final generation receipt](independent-catalog-reproduction-final-lookup.json).

The helper database negative is a real local SQL check, not a complete successful RDS IAM/TLS/S3/KMS readiness trial. The managed, native, source/image CI and hosted gates listed above remain independently required. No cloud, Git, ticket-status or deployment action was performed by this reviewer.
