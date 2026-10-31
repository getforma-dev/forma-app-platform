//! Isolated, explicitly armed test executable only. Never a service entry.
//!
//! APP-075 (ADR-005) owns this entry. It is compiled only into the `cfg(test)`
//! library test executable and runs the shared application-data-read-only
//! cases through production `Settings::new`/`Stores` against the fixed RDS
//! hostname, port 5432 and the CA at its fixed path. It exposes no endpoint,
//! role, SQL, CA path or transport override.
//!
//! # Invocation
//!
//! Exactly `<test-executable> --exact
//! storage::managed_qualification::managed_readonly_entry --ignored
//! --nocapture --test-threads=1`, as UID 65532, with
//! `GENUI_APP008_MANAGED_QUALIFICATION=APP-008`, the closed run manifest, the
//! byte-exact synthetic registry and the credential file owned 0600 in the
//! 0700 `/run/genui`, and the CA at `/etc/ssl/certs/rds-global-bundle.pem`.
//!
//! # Original deadline
//!
//! The manifest's `issued_at`/`expires_at` span at most 600 s; `issued_at` may
//! lead this clock by at most 5 s (as `config.rs` allows) and at least 60 s
//! must remain when the manifest is validated. The wall clock and the
//! monotonic clock are sampled together before any file is read, so validation
//! time never renews the deadline. Cases stop 4 s before the original expiry,
//! cancellation must be acknowledged 3 s before it and pool closure must finish
//! 1 s before it. A watchdog thread exits the process with status 124 at the
//! original expiry whatever the main thread is doing (including a blocked
//! stdout write); that exit emits no result line.
//!
//! # Output (stdout-drain contract)
//!
//! stdout carries libtest's own lines, `APP008 check: <name>` progress lines
//! (the first shares libtest's `test <name> ... ` line) and at most one final
//! line beginning `APP008_MANAGED_RESULT <json>`, preceded by a newline, whose
//! JSON is at most 8,192 bytes. Readers select lines by that prefix. The reader (the qualification supervisor or the local
//! harness) must drain stdout and stderr continuously until EOF: `println!`
//! blocks on a full pipe, and a stalled reader therefore ends in the watchdog
//! exit without a result. A missing result line is a failure. stderr is not
//! part of the contract; a process-wide panic hook installed before anything
//! else suppresses every panic payload, and task panic payloads are dropped.
//!
//! # Cleanup receipt
//!
//! `cleanup_confirmed` is true only when the case task finished (or its abort
//! was acknowledged), `Stores::close` succeeded and all eight transport
//! permits are free. Refusals before store construction report
//! `cleanup_applicable:false`. A non-case (entry) panic reports `entry_panic`
//! with cleanup unconfirmed; once the manifest has been validated the
//! watchdog is re-armed at the original deadline until that line is written.
//! `passed:true` requires confirmed cleanup and exactly the six named
//! [`CHECKS`], in order, each passed.
use super::{pool, qualification_cases as cases, valid_id, Settings, Stores, TrustedRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    future::Future,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Component, Path, PathBuf},
    sync::{mpsc, Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{timeout_at, Instant};

const ENTRY: &str = "storage::managed_qualification::managed_readonly_entry";
const ARGUMENTS: [&str; 5] = [
    "--exact",
    ENTRY,
    "--ignored",
    "--nocapture",
    "--test-threads=1",
];
const SCOPE: &str = "APP-008 isolated application-data-read-only qualification";
const ARM: &str = "GENUI_APP008_MANAGED_QUALIFICATION";
const ARM_VALUE: &str = "APP-008";
const UID: u32 = 65532;
const HOST: &str = "genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com";
const RESOURCE_ID: &str = "db-KW4LRMTPI5YXHZXF4LELQZKTAU";
const ISSUER: &str = "app008-fixture-only";
const MANIFEST: &str = "/run/genui/app008-managed-run.json";
const CREDENTIALS: &str = "/run/genui/credentials.json";
const REGISTRY: &str = "/run/genui/app008-managed-registry.json";
const CA: &str = "/etc/ssl/certs/rds-global-bundle.pem";
/// Seed provenance as (pin field, repository path, declared SHA-256), in pin
/// order: the fixture setup, the two contract fixtures of edition
/// `impl-2026-10-28.1` that `tenant_seed.make_bundles` reads, the seed program
/// itself and the contract validator whose `digest` it runs. This is the one
/// path list: unit tests compare each declared hash with the repository file
/// and the edition with `tenant_seed.py`, and `local_harness.py` (and through
/// it `check_image.py`) reads this table. The executable cannot prove which
/// bytes seeded the target, so these are declared pins the operator must
/// match; the rest of the operator's seed environment is bound by APP-077's
/// operator-image digest.
const SEED_INPUTS: [(&str, &str, &str); 6] = [
    (
        "fixture_setup_sha256",
        "fixtures/phase-one/v1/setup.json",
        "7460864468915d3ca1660303e2569e0eb96837e6b6fd703e91be0f92b984ec7c",
    ),
    (
        "seed_northstar_sha256",
        "contracts/fixtures/impl-2026-10-28.1/northstar.json",
        "45639ea369193d60f45fbafbe6c3aad648881017df57615bdf89bd501dfdd592",
    ),
    (
        "seed_spend_review_sha256",
        "contracts/fixtures/impl-2026-10-28.1/spend-review.json",
        "4a4581f5a7fcfec867070584a00b0fbebdf33558fffe538eec35f97f72aa5650",
    ),
    (
        "seed_program_sha256",
        "app/runtime/tenant_seed.py",
        "b8bbd34bbdc48b642ddefef057ed245ee5047bbb9eb58820208b8d706cc15c8f",
    ),
    (
        "seed_validator_sha256",
        "contracts/validate.py",
        "666f9d3626f2cb7d8998e0287631895785b85c007f99a33059a758ec42aacfaf",
    ),
    // ADR-011 library (APP-094): the catalog pins every library fixture the
    // `seed-library` command places (each entry's `fixture_sha256`).
    (
        "seed_library_sha256",
        "contracts/fixtures/library/catalog.json",
        "fb92b15e761c84c593dcd1a8a19ee4077f672f40fc0ba05dbf7ee7e889c44828",
    ),
];
/// The shared cases' checks a pass requires, in order (`qualification_cases`).
const CHECKS: [&str; 6] = [
    "aggregate fixed-role native readiness verifies identity, schema and blank scope",
    "TLS role identity and scoped two-organization publication/record reads",
    "all four fixed roles enforce RLS, blank scope and direct write refusal",
    "revocation, epoch, missing grants and registry/DB binding mismatches deny",
    "closed action matrix evaluates current scoped grants without granting business writes",
    "same-backend commit/error-rollback reuse clears poisoned role/settings, listeners and advisory locks",
];
const QUERY: &[u8] = include_bytes!("../../migrations/catalog-query.json");
const CATALOG: &[u8] = include_bytes!("../../migrations/catalog-v15.json");
const MIGRATIONS: &[u8] = include_bytes!("../../migrations/manifest.json");
const MANIFEST_LIMIT: u64 = 8192;
const REGISTRY_LIMIT: u64 = 16384;
const CREDENTIALS_LIMIT: u64 = 65536;
const CA_LIMIT: u64 = 1024 * 1024;
const MAX_SPAN: u64 = 600;
const ISSUED_SKEW: u64 = 5;
const MIN_REMAINING: Duration = Duration::from_secs(60);
const CASE_RESERVE: Duration = Duration::from_secs(4);
const ABORT_RESERVE: Duration = Duration::from_secs(3);
const CLOSE_RESERVE: Duration = Duration::from_secs(1);
const RESULT_PREFIX: &str = "APP008_MANAGED_RESULT";
const RESULT_LIMIT: usize = 8192;

/// Every compile-time input of the library test executable, in this order:
/// the Rust sources and Cargo files, plus every file a source embeds with
/// `include_str!`/`include_bytes!` (the RDS CA and the contract files the
/// tests embed). Only the embedded catalog/migration resources are pinned
/// separately below. This is a binding of compiled inputs, not a whole-image
/// or Git-revision attestation.
const SOURCES: [(&str, &[u8]); 183] = [
    ("app/Cargo.lock", include_bytes!("../../Cargo.lock")),
    ("app/Cargo.toml", include_bytes!("../../Cargo.toml")),
    (
        "app/certs/rds-global-bundle.pem",
        include_bytes!("../../certs/rds-global-bundle.pem"),
    ),
    (
        "app/src/actions/ledger.rs",
        include_bytes!("../actions/ledger.rs"),
    ),
    (
        "app/src/actions/mod.rs",
        include_bytes!("../actions/mod.rs"),
    ),
    (
        "app/src/actions/prompt.rs",
        include_bytes!("../actions/prompt.rs"),
    ),
    (
        "app/src/actions/receipt.rs",
        include_bytes!("../actions/receipt.rs"),
    ),
    (
        "app/src/actions/receipt_tests.rs",
        include_bytes!("../actions/receipt_tests.rs"),
    ),
    (
        "app/src/actions/rehearsed.rs",
        include_bytes!("../actions/rehearsed.rs"),
    ),
    (
        "app/src/actions/resolve.rs",
        include_bytes!("../actions/resolve.rs"),
    ),
    (
        "app/src/actions/routes.rs",
        include_bytes!("../actions/routes.rs"),
    ),
    (
        "app/src/actions/tests.rs",
        include_bytes!("../actions/tests.rs"),
    ),
    (
        "app/src/actions/workflows.rs",
        include_bytes!("../actions/workflows.rs"),
    ),
    (
        "app/src/admission/canonical.rs",
        include_bytes!("../admission/canonical.rs"),
    ),
    (
        "app/src/admission/digest.rs",
        include_bytes!("../admission/digest.rs"),
    ),
    (
        "app/src/admission/mod.rs",
        include_bytes!("../admission/mod.rs"),
    ),
    (
        "app/src/admission/tests.rs",
        include_bytes!("../admission/tests.rs"),
    ),
    (
        "app/src/admission/validate.rs",
        include_bytes!("../admission/validate.rs"),
    ),
    (
        "app/src/auth/aliases.rs",
        include_bytes!("../auth/aliases.rs"),
    ),
    (
        "app/src/auth/capability.rs",
        include_bytes!("../auth/capability.rs"),
    ),
    (
        "app/src/auth/cookies.rs",
        include_bytes!("../auth/cookies.rs"),
    ),
    ("app/src/auth/csrf.rs", include_bytes!("../auth/csrf.rs")),
    (
        "app/src/auth/invite.rs",
        include_bytes!("../auth/invite.rs"),
    ),
    (
        "app/src/auth/invite_tests.rs",
        include_bytes!("../auth/invite_tests.rs"),
    ),
    ("app/src/auth/mod.rs", include_bytes!("../auth/mod.rs")),
    (
        "app/src/auth/pending.rs",
        include_bytes!("../auth/pending.rs"),
    ),
    ("app/src/auth/pkce.rs", include_bytes!("../auth/pkce.rs")),
    (
        "app/src/auth/provider.rs",
        include_bytes!("../auth/provider.rs"),
    ),
    (
        "app/src/auth/routes.rs",
        include_bytes!("../auth/routes.rs"),
    ),
    (
        "app/src/auth/synthetic_issuer.rs",
        include_bytes!("../auth/synthetic_issuer.rs"),
    ),
    ("app/src/auth/tests.rs", include_bytes!("../auth/tests.rs")),
    ("app/src/auth/vault.rs", include_bytes!("../auth/vault.rs")),
    (
        "app/src/auth_qualification.rs",
        include_bytes!("../auth_qualification.rs"),
    ),
    (
        "app/src/auth_qualification/tests.rs",
        include_bytes!("../auth_qualification/tests.rs"),
    ),
    (
        "app/src/bin/genui-dev.rs",
        include_bytes!("../bin/genui-dev.rs"),
    ),
    ("app/src/boxes/ask.rs", include_bytes!("../boxes/ask.rs")),
    (
        "app/src/boxes/brief.rs",
        include_bytes!("../boxes/brief.rs"),
    ),
    (
        "app/src/boxes/derive.rs",
        include_bytes!("../boxes/derive.rs"),
    ),
    (
        "app/src/boxes/explain.rs",
        include_bytes!("../boxes/explain.rs"),
    ),
    (
        "app/src/boxes/inspect.rs",
        include_bytes!("../boxes/inspect.rs"),
    ),
    (
        "app/src/boxes/intent.rs",
        include_bytes!("../boxes/intent.rs"),
    ),
    ("app/src/boxes/mod.rs", include_bytes!("../boxes/mod.rs")),
    (
        "app/src/boxes/register.rs",
        include_bytes!("../boxes/register.rs"),
    ),
    (
        "app/src/boxes/routes.rs",
        include_bytes!("../boxes/routes.rs"),
    ),
    (
        "app/src/boxes/sniff.rs",
        include_bytes!("../boxes/sniff.rs"),
    ),
    (
        "app/src/boxes/store.rs",
        include_bytes!("../boxes/store.rs"),
    ),
    (
        "app/src/boxes/tests.rs",
        include_bytes!("../boxes/tests.rs"),
    ),
    (
        "app/src/commands/mod.rs",
        include_bytes!("../commands/mod.rs"),
    ),
    (
        "app/src/commands/payload.rs",
        include_bytes!("../commands/payload.rs"),
    ),
    (
        "app/src/commands/routes.rs",
        include_bytes!("../commands/routes.rs"),
    ),
    (
        "app/src/commands/tests.rs",
        include_bytes!("../commands/tests.rs"),
    ),
    (
        "app/src/composer/ask.rs",
        include_bytes!("../composer/ask.rs"),
    ),
    (
        "app/src/composer/cards.rs",
        include_bytes!("../composer/cards.rs"),
    ),
    (
        "app/src/composer/commands.rs",
        include_bytes!("../composer/commands.rs"),
    ),
    (
        "app/src/composer/labels.rs",
        include_bytes!("../composer/labels.rs"),
    ),
    (
        "app/src/composer/lineage.rs",
        include_bytes!("../composer/lineage.rs"),
    ),
    (
        "app/src/composer/matcher.rs",
        include_bytes!("../composer/matcher.rs"),
    ),
    (
        "app/src/composer/mod.rs",
        include_bytes!("../composer/mod.rs"),
    ),
    (
        "app/src/composer/model.rs",
        include_bytes!("../composer/model.rs"),
    ),
    (
        "app/src/composer/plan.rs",
        include_bytes!("../composer/plan.rs"),
    ),
    (
        "app/src/composer/prompt.rs",
        include_bytes!("../composer/prompt.rs"),
    ),
    (
        "app/src/composer/routes.rs",
        include_bytes!("../composer/routes.rs"),
    ),
    (
        "app/src/composer/shape.rs",
        include_bytes!("../composer/shape.rs"),
    ),
    (
        "app/src/composer/talk.rs",
        include_bytes!("../composer/talk.rs"),
    ),
    (
        "app/src/composer/tests.rs",
        include_bytes!("../composer/tests.rs"),
    ),
    (
        "app/src/composer/transform.rs",
        include_bytes!("../composer/transform.rs"),
    ),
    ("app/src/config.rs", include_bytes!("../config.rs")),
    ("app/src/demo.rs", include_bytes!("../demo.rs")),
    ("app/src/dev/db.rs", include_bytes!("../dev/db.rs")),
    ("app/src/dev/mod.rs", include_bytes!("../dev/mod.rs")),
    (
        "app/src/dev/personas.rs",
        include_bytes!("../dev/personas.rs"),
    ),
    ("app/src/dev/seed.rs", include_bytes!("../dev/seed.rs")),
    ("app/src/directory.rs", include_bytes!("../directory.rs")),
    ("app/src/events/mod.rs", include_bytes!("../events/mod.rs")),
    (
        "app/src/events/routes.rs",
        include_bytes!("../events/routes.rs"),
    ),
    (
        "app/src/events/tests.rs",
        include_bytes!("../events/tests.rs"),
    ),
    (
        "app/src/http/limits.rs",
        include_bytes!("../http/limits.rs"),
    ),
    ("app/src/http/mod.rs", include_bytes!("../http/mod.rs")),
    ("app/src/http/proxy.rs", include_bytes!("../http/proxy.rs")),
    (
        "app/src/imports/mod.rs",
        include_bytes!("../imports/mod.rs"),
    ),
    (
        "app/src/imports/sandbox.rs",
        include_bytes!("../imports/sandbox.rs"),
    ),
    ("app/src/lib.rs", include_bytes!("../lib.rs")),
    ("app/src/mcp/drafts.rs", include_bytes!("../mcp/drafts.rs")),
    (
        "app/src/mcp/governance.rs",
        include_bytes!("../mcp/governance.rs"),
    ),
    ("app/src/mcp/mod.rs", include_bytes!("../mcp/mod.rs")),
    ("app/src/mcp/scope.rs", include_bytes!("../mcp/scope.rs")),
    ("app/src/mcp/tests.rs", include_bytes!("../mcp/tests.rs")),
    ("app/src/mcp/tokens.rs", include_bytes!("../mcp/tokens.rs")),
    ("app/src/mcp/tools.rs", include_bytes!("../mcp/tools.rs")),
    ("app/src/origins.rs", include_bytes!("../origins.rs")),
    ("app/src/pages.rs", include_bytes!("../pages.rs")),
    ("app/src/product.rs", include_bytes!("../product.rs")),
    (
        "app/src/query/canvas.rs",
        include_bytes!("../query/canvas.rs"),
    ),
    (
        "app/src/query/canvas_display.rs",
        include_bytes!("../query/canvas_display.rs"),
    ),
    (
        "app/src/query/canvas_ledger.rs",
        include_bytes!("../query/canvas_ledger.rs"),
    ),
    (
        "app/src/query/canvas_tests.rs",
        include_bytes!("../query/canvas_tests.rs"),
    ),
    ("app/src/query/mod.rs", include_bytes!("../query/mod.rs")),
    ("app/src/query/page.rs", include_bytes!("../query/page.rs")),
    (
        "app/src/query/receipt.rs",
        include_bytes!("../query/receipt.rs"),
    ),
    (
        "app/src/query/route_tests.rs",
        include_bytes!("../query/route_tests.rs"),
    ),
    (
        "app/src/query/routes.rs",
        include_bytes!("../query/routes.rs"),
    ),
    (
        "app/src/query/tests.rs",
        include_bytes!("../query/tests.rs"),
    ),
    (
        "app/src/query/worker.rs",
        include_bytes!("../query/worker.rs"),
    ),
    ("app/src/readiness.rs", include_bytes!("../readiness.rs")),
    ("app/src/roles.rs", include_bytes!("../roles.rs")),
    (
        "app/src/sharing/mod.rs",
        include_bytes!("../sharing/mod.rs"),
    ),
    ("app/src/slack/api.rs", include_bytes!("../slack/api.rs")),
    (
        "app/src/slack/flows.rs",
        include_bytes!("../slack/flows.rs"),
    ),
    ("app/src/slack/link.rs", include_bytes!("../slack/link.rs")),
    ("app/src/slack/mod.rs", include_bytes!("../slack/mod.rs")),
    (
        "app/src/slack/notify.rs",
        include_bytes!("../slack/notify.rs"),
    ),
    (
        "app/src/slack/signature.rs",
        include_bytes!("../slack/signature.rs"),
    ),
    (
        "app/src/slack/tests.rs",
        include_bytes!("../slack/tests.rs"),
    ),
    ("app/src/slack/text.rs", include_bytes!("../slack/text.rs")),
    ("app/src/storage/actions.rs", include_bytes!("actions.rs")),
    (
        "app/src/storage/actions_campaign.rs",
        include_bytes!("actions_campaign.rs"),
    ),
    (
        "app/src/storage/admission.rs",
        include_bytes!("admission.rs"),
    ),
    (
        "app/src/storage/admission_qualification.rs",
        include_bytes!("admission_qualification.rs"),
    ),
    ("app/src/storage/agents.rs", include_bytes!("agents.rs")),
    (
        "app/src/storage/authorization.rs",
        include_bytes!("authorization.rs"),
    ),
    (
        "app/src/storage/box_campaign.rs",
        include_bytes!("box_campaign.rs"),
    ),
    ("app/src/storage/boxes.rs", include_bytes!("boxes.rs")),
    (
        "app/src/storage/canvas_campaign.rs",
        include_bytes!("canvas_campaign.rs"),
    ),
    ("app/src/storage/cards.rs", include_bytes!("cards.rs")),
    ("app/src/storage/catalog.rs", include_bytes!("catalog.rs")),
    (
        "app/src/storage/command_qualification.rs",
        include_bytes!("command_qualification.rs"),
    ),
    ("app/src/storage/commands.rs", include_bytes!("commands.rs")),
    ("app/src/storage/composer.rs", include_bytes!("composer.rs")),
    (
        "app/src/storage/composer_qualification.rs",
        include_bytes!("composer_qualification.rs"),
    ),
    (
        "app/src/storage/definition_qualification.rs",
        include_bytes!("definition_qualification.rs"),
    ),
    (
        "app/src/storage/definitions.rs",
        include_bytes!("definitions.rs"),
    ),
    (
        "app/src/storage/governance.rs",
        include_bytes!("governance.rs"),
    ),
    ("app/src/storage/labels.rs", include_bytes!("labels.rs")),
    (
        "app/src/storage/managed_qualification.rs",
        include_bytes!("managed_qualification.rs"),
    ),
    (
        "app/src/storage/managed_qualification/tests.rs",
        include_bytes!("managed_qualification/tests.rs"),
    ),
    (
        "app/src/storage/mcp_campaign.rs",
        include_bytes!("mcp_campaign.rs"),
    ),
    ("app/src/storage/mod.rs", include_bytes!("mod.rs")),
    (
        "app/src/storage/pitch_qualification.rs",
        include_bytes!("pitch_qualification.rs"),
    ),
    ("app/src/storage/pool.rs", include_bytes!("pool.rs")),
    (
        "app/src/storage/qualification.rs",
        include_bytes!("qualification.rs"),
    ),
    (
        "app/src/storage/qualification_cases.rs",
        include_bytes!("qualification_cases.rs"),
    ),
    ("app/src/storage/queries.rs", include_bytes!("queries.rs")),
    (
        "app/src/storage/query_qualification.rs",
        include_bytes!("query_qualification.rs"),
    ),
    (
        "app/src/storage/race_qualification.rs",
        include_bytes!("race_qualification.rs"),
    ),
    ("app/src/storage/registry.rs", include_bytes!("registry.rs")),
    ("app/src/storage/rules.rs", include_bytes!("rules.rs")),
    ("app/src/storage/sessions.rs", include_bytes!("sessions.rs")),
    ("app/src/storage/sharing.rs", include_bytes!("sharing.rs")),
    (
        "app/src/storage/sharing_qualification.rs",
        include_bytes!("sharing_qualification.rs"),
    ),
    (
        "app/src/storage/slack_campaign.rs",
        include_bytes!("slack_campaign.rs"),
    ),
    ("app/src/storage/tests.rs", include_bytes!("tests.rs")),
    (
        "app/src/storage/workflows_campaign.rs",
        include_bytes!("workflows_campaign.rs"),
    ),
    ("app/src/supervise.rs", include_bytes!("../supervise.rs")),
    (
        "app/src/widgets/customize.rs",
        include_bytes!("../widgets/customize.rs"),
    ),
    (
        "app/src/widgets/mod.rs",
        include_bytes!("../widgets/mod.rs"),
    ),
    (
        "app/src/widgets/routes.rs",
        include_bytes!("../widgets/routes.rs"),
    ),
    (
        "app/src/widgets/state.rs",
        include_bytes!("../widgets/state.rs"),
    ),
    (
        "app/src/widgets/tests.rs",
        include_bytes!("../widgets/tests.rs"),
    ),
    (
        "app/src/widgets/weather.rs",
        include_bytes!("../widgets/weather.rs"),
    ),
    (
        "contracts/fixtures/canonical-vectors.json",
        include_bytes!("../../../contracts/fixtures/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/command-recovery.json",
        include_bytes!("../../../contracts/fixtures/command-recovery.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/negative-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-28.1/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/northstar.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-28.1/northstar.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/spend-review.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/canonical-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.2/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/negative-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.2/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/northstar.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.2/northstar.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/spend-review.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.2/spend-review.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.3/canonical-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.3/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.3/command-recovery.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.3/command-recovery.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.3/negative-vectors.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.3/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.3/northstar.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.3/northstar.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.3/spend-review.json",
        include_bytes!("../../../contracts/fixtures/impl-2026-10-29.3/spend-review.json"),
    ),
    (
        "contracts/fixtures/library/catalog.json",
        include_bytes!("../../../contracts/fixtures/library/catalog.json"),
    ),
    (
        "contracts/fixtures/negative-vectors.json",
        include_bytes!("../../../contracts/fixtures/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/northstar.json",
        include_bytes!("../../../contracts/fixtures/northstar.json"),
    ),
    (
        "contracts/fixtures/spend-review.json",
        include_bytes!("../../../contracts/fixtures/spend-review.json"),
    ),
    (
        "contracts/fixtures/stored/expense-claims-d70710d3.json",
        include_bytes!("../../../contracts/fixtures/stored/expense-claims-d70710d3.json"),
    ),
    (
        "contracts/implementation-lock.json",
        include_bytes!("../../../contracts/implementation-lock.json"),
    ),
    (
        "contracts/schema.json",
        include_bytes!("../../../contracts/schema.json"),
    ),
    (
        "spikes/APP-006/Cargo.toml",
        include_bytes!("../../../spikes/APP-006/Cargo.toml"),
    ),
    (
        "spikes/APP-006/src/lib.rs",
        include_bytes!("../../../spikes/APP-006/src/lib.rs"),
    ),
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    source_bundle_sha256: String,
    fixture_setup_sha256: String,
    seed_northstar_sha256: String,
    seed_spend_review_sha256: String,
    seed_program_sha256: String,
    seed_validator_sha256: String,
    seed_library_sha256: String,
    catalog_query_sha256: String,
    database_version: u32,
    catalog_sha256: String,
    migrations_manifest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    scope: String,
    armed: bool,
    run_id: String,
    issued_at: u64,
    expires_at: u64,
    database_host: String,
    database_port: u16,
    database_name: String,
    declared_rds_resource_id: String,
    organizations: [String; 2],
    registry_sha256: String,
    ca_sha256: String,
    pins: Pins,
}

struct Validated {
    manifest: Manifest,
    manifest_sha256: String,
    deadline: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Configuration,
    Case,
    Deadline,
    Cleanup,
    EntryPanic,
}
impl Failure {
    fn code(self) -> &'static str {
        match self {
            Self::Configuration => "configuration_refused",
            Self::Case => "case_refused",
            Self::Deadline => "original_deadline_reached",
            Self::Cleanup => "cleanup_unconfirmed",
            Self::EntryPanic => "entry_panic",
        }
    }
}
type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cleanup {
    /// Refused before any store (and therefore transport) could exist.
    NotApplicable,
    Confirmed,
    Unconfirmed,
}

/// Fixed in the entry; only unit tests substitute private temporary paths.
struct Paths {
    manifest: PathBuf,
    credentials: PathBuf,
    registry: PathBuf,
    ca: PathBuf,
}
impl Paths {
    fn fixed() -> Self {
        Self {
            manifest: MANIFEST.into(),
            credentials: CREDENTIALS.into(),
            registry: REGISTRY.into(),
            ca: CA.into(),
        }
    }
}

/// Process facts sampled once at entry: arguments, effective UID, arming
/// value and the paired wall/monotonic clock samples.
struct Environment {
    args: Vec<String>,
    euid: u32,
    arm: Option<String>,
    now: SystemTime,
    monotonic: Instant,
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Failure::Configuration)
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
/// Name + NUL + lowercase file SHA-256 + newline per file, then SHA-256.
fn source_bundle_sha256() -> String {
    let mut binding = Vec::new();
    for (name, raw) in SOURCES {
        binding.extend_from_slice(name.as_bytes());
        binding.push(0);
        binding.extend_from_slice(digest(raw).as_bytes());
        binding.push(b'\n');
    }
    digest(&binding)
}
fn database_version() -> u32 {
    serde_json::from_slice::<Value>(MIGRATIONS)
        .ok()
        .and_then(|value| value["database_version"].as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}
fn expected_pins() -> Pins {
    Pins {
        source_bundle_sha256: source_bundle_sha256(),
        fixture_setup_sha256: SEED_INPUTS[0].2.into(),
        seed_northstar_sha256: SEED_INPUTS[1].2.into(),
        seed_spend_review_sha256: SEED_INPUTS[2].2.into(),
        seed_program_sha256: SEED_INPUTS[3].2.into(),
        seed_validator_sha256: SEED_INPUTS[4].2.into(),
        seed_library_sha256: SEED_INPUTS[5].2.into(),
        catalog_query_sha256: digest(QUERY),
        database_version: database_version(),
        catalog_sha256: digest(CATALOG),
        migrations_manifest_sha256: digest(MIGRATIONS),
    }
}
/// Canonical synthetic registry: serde_json object keys are sorted, compact.
fn registry_bytes(run: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":1,"registry_version":run,"bindings":[
        {"issuer":ISSUER,"provider_tenant_id":format!("ten_{}",cases::ORG),"organization_id":cases::ORG},
        {"issuer":ISSUER,"provider_tenant_id":format!("ten_{}",cases::OTHER),"organization_id":cases::OTHER}
    ]})).expect("fixed registry serialization")
}

/// Reads one input without following links or blocking on FIFOs. Private
/// inputs must be owned by `uid`, mode 0600, in a 0700 directory owned by
/// `uid`; the public CA must not be group/world writable. Every input must be
/// a single-link regular file within `limit` bytes under real directories.
fn bytes(path: &Path, limit: u64, private: bool, uid: u32) -> Result<Vec<u8>> {
    require(
        path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
    )?;
    for parent in path.ancestors().skip(1) {
        require(
            parent
                .symlink_metadata()
                .map_err(|_| Failure::Configuration)?
                .is_dir(),
        )?;
    }
    if private {
        let parent = path
            .parent()
            .ok_or(Failure::Configuration)?
            .symlink_metadata()
            .map_err(|_| Failure::Configuration)?;
        require(parent.uid() == uid && parent.mode() & 0o777 == 0o700)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Failure::Configuration)?;
    let metadata = file.metadata().map_err(|_| Failure::Configuration)?;
    require(metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit)?;
    if private {
        require(metadata.uid() == uid && metadata.mode() & 0o777 == 0o600)?;
    } else {
        require(metadata.mode() & 0o022 == 0)?;
    }
    let mut raw = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut raw)
        .map_err(|_| Failure::Configuration)?;
    require(raw.len() as u64 <= limit)?;
    Ok(raw)
}

/// Closed manifest: unknown or duplicate keys, a wrong selector/target or
/// pin, or an original interval outside the bounds refuse the whole run.
fn parse(raw: &[u8], now: SystemTime, monotonic: Instant) -> Result<Validated> {
    require(!raw.is_empty() && raw.len() as u64 <= MANIFEST_LIMIT)?;
    let manifest: Manifest = serde_json::from_slice(raw).map_err(|_| Failure::Configuration)?;
    require(manifest.schema_version == 1 && manifest.scope == SCOPE && manifest.armed)?;
    require(valid_id(&manifest.run_id) && manifest.run_id.as_bytes()[14] == b'4')?;
    let now_seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::Configuration)?
        .as_secs();
    require(
        manifest.issued_at <= now_seconds.saturating_add(ISSUED_SKEW)
            && manifest.expires_at > manifest.issued_at
            && manifest.expires_at - manifest.issued_at <= MAX_SPAN,
    )?;
    let expiry = UNIX_EPOCH
        .checked_add(Duration::from_secs(manifest.expires_at))
        .ok_or(Failure::Configuration)?;
    let remaining = expiry
        .duration_since(now)
        .map_err(|_| Failure::Configuration)?;
    require(
        remaining >= MIN_REMAINING && remaining <= Duration::from_secs(MAX_SPAN + ISSUED_SKEW),
    )?;
    require(
        manifest.database_host == HOST
            && manifest.database_port == 5432
            && manifest.database_name == "genui"
            && manifest.declared_rds_resource_id == RESOURCE_ID
            && manifest.organizations == [cases::ORG, cases::OTHER],
    )?;
    require(manifest.pins == expected_pins())?;
    require(manifest.registry_sha256 == digest(&registry_bytes(&manifest.run_id)))?;
    require(lower_hex_sha256(&manifest.ca_sha256))?;
    Ok(Validated {
        manifest,
        manifest_sha256: digest(raw),
        deadline: monotonic + remaining,
    })
}

fn load(environment: &Environment) -> Result<Validated> {
    load_as(environment, &Paths::fixed(), UID)
}
fn load_as(environment: &Environment, paths: &Paths, uid: u32) -> Result<Validated> {
    require(environment.euid == uid)?;
    require(environment.arm.as_deref() == Some(ARM_VALUE))?;
    let raw = bytes(&paths.manifest, MANIFEST_LIMIT, true, uid)?;
    let validated = parse(&raw, environment.now, environment.monotonic)?;
    validate_files(&validated, paths, uid)?;
    Ok(validated)
}
/// Re-run after the cases as well: a replaced input voids the receipt.
fn validate_files(validated: &Validated, paths: &Paths, uid: u32) -> Result<()> {
    require(
        digest(&bytes(&paths.manifest, MANIFEST_LIMIT, true, uid)?) == validated.manifest_sha256,
    )?;
    require(
        bytes(&paths.registry, REGISTRY_LIMIT, true, uid)?
            == registry_bytes(&validated.manifest.run_id),
    )?;
    require(digest(&bytes(&paths.ca, CA_LIMIT, false, uid)?) == validated.manifest.ca_sha256)?;
    bytes(&paths.credentials, CREDENTIALS_LIMIT, true, uid)?;
    let config =
        crate::config::Config::load(&paths.credentials).map_err(|_| Failure::Configuration)?;
    require(config.database_host == HOST && config.database_name == "genui")?;
    require(
        SystemTime::now() < UNIX_EPOCH + Duration::from_secs(validated.manifest.expires_at)
            && Instant::now() < validated.deadline,
    )?;
    Ok(())
}

/// Exits the whole process with status 124 at the original deadline.
struct Watchdog(
    Option<mpsc::SyncSender<()>>,
    Option<std::thread::JoinHandle<()>>,
);
impl Watchdog {
    fn start(deadline: Instant) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let thread = std::thread::Builder::new()
            .name("app008-original-deadline".into())
            .spawn(move || {
                if let Err(mpsc::RecvTimeoutError::Timeout) = receiver.recv_timeout(remaining) {
                    std::process::exit(124);
                }
            })
            .map_err(|_| Failure::Configuration)?;
        Ok(Self(Some(sender), Some(thread)))
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.try_send(());
        }
        if let Some(thread) = self.1.take() {
            let _ = thread.join();
        }
    }
}

fn before(deadline: Instant, reserve: Duration) -> Instant {
    deadline.checked_sub(reserve).unwrap_or_else(Instant::now)
}

/// Constructs the stores, runs `cases` in one task bounded by the original
/// deadline and proves cleanup. Only a finished (or abort-acknowledged) task,
/// a successful `Stores::close` and eight free transport permits confirm it.
async fn supervise<F, Fut>(
    settings: std::result::Result<Settings, super::Error>,
    deadline: Instant,
    cases: F,
    recheck: impl FnOnce() -> Result<()>,
) -> (Result<Vec<Value>>, Cleanup)
where
    F: FnOnce(Arc<Stores>) -> Fut,
    Fut: Future<Output = Vec<Value>> + Send + 'static,
{
    let Ok(settings) = settings else {
        return (Err(Failure::Configuration), Cleanup::NotApplicable);
    };
    let stores = match Stores::new(settings) {
        Ok(stores) => Arc::new(stores),
        Err(_) => return (Err(Failure::Configuration), Cleanup::NotApplicable),
    };
    let mut task = tokio::spawn(cases(stores.clone()));
    let (result, task_finished) = match timeout_at(before(deadline, CASE_RESERVE), &mut task).await
    {
        Ok(Ok(checks)) => (Ok(checks), true),
        // The JoinError (and any panic payload in it) is dropped unread.
        Ok(Err(_)) => (Err(Failure::Case), true),
        Err(_) => {
            task.abort();
            // Await cancellation before closing pools; no detached owner may
            // survive while a successful cleanup receipt is emitted.
            let finished = timeout_at(before(deadline, ABORT_RESERVE), &mut task)
                .await
                .is_ok();
            (Err(Failure::Deadline), finished)
        }
    };
    let closed = matches!(
        timeout_at(before(deadline, CLOSE_RESERVE), stores.close()).await,
        Ok(Ok(()))
    );
    let cleanup = if task_finished && closed && pool::slots().0 == 0 {
        Cleanup::Confirmed
    } else {
        Cleanup::Unconfirmed
    };
    let result = if cleanup != Cleanup::Confirmed {
        Err(Failure::Cleanup)
    } else if recheck().is_err() {
        Err(Failure::Configuration)
    } else {
        result
    };
    (result, cleanup)
}

async fn production_cases(stores: Arc<Stores>) -> Vec<Value> {
    let mut checks = Vec::new();
    cases::readiness(&stores, &mut checks).await;
    let registry_sha256 = stores.registry_identity().sha256.clone();
    cases::read_only_and_reuse(
        &stores,
        |organization, principal, epoch| TrustedRequest {
            organization: organization.into(),
            principal: principal.into(),
            epoch,
            issuer: ISSUER.into(),
            provider_tenant: format!("ten_{organization}"),
            provider_subject: format!("usr_{principal}"),
            registry_sha256: registry_sha256.clone(),
        },
        &mut checks,
    )
    .await;
    checks
}

/// Exactly the six named checks, in order, each `{"name":..,"passed":true}`.
fn complete(checks: &[Value]) -> bool {
    checks.len() == CHECKS.len()
        && checks
            .iter()
            .zip(CHECKS)
            .all(|(check, name)| *check == json!({"name":name,"passed":true}))
}

fn report(validated: Option<&Validated>, result: Result<Vec<Value>>, cleanup: Cleanup) -> Value {
    let (passed, checks, error) = match result {
        Ok(_) if cleanup != Cleanup::Confirmed => {
            (false, Vec::new(), Some(Failure::Cleanup.code()))
        }
        Ok(checks) if complete(&checks) => (true, checks, None),
        Ok(_) => (false, Vec::new(), Some(Failure::Case.code())),
        Err(error) => (false, Vec::new(), Some(error.code())),
    };
    json!({"schema_version":1,"scope":SCOPE,"passed":passed,"error":error,"checks":checks,
        "cleanup_applicable":cleanup != Cleanup::NotApplicable,
        "cleanup_confirmed":cleanup == Cleanup::Confirmed,
        "run_id":validated.map(|v|&v.manifest.run_id),
        "original_issued_at":validated.map(|v|v.manifest.issued_at),"original_expires_at":validated.map(|v|v.manifest.expires_at),
        "manifest_sha256":validated.map(|v|&v.manifest_sha256),"resource_pins":expected_pins(),
        "declared_control_plane_resource_id":RESOURCE_ID,
        "ca_sha256_self_consistency_only":validated.map(|v|&v.manifest.ca_sha256),
        "limits":["Fixture identities are synthetic; no hosted identity or application admission is established.",
        "RDS resource ID and CA provenance require separate reviewed control-plane and immutable-image receipts.",
        "This subset does not pass managed races, drift, HTTP, recovery or complete APP-008 acceptance."]})
}

/// The whole entry after argument capture: selector, arming, inputs,
/// watchdog, runtime, cases, cleanup and the bounded report. The validated
/// original deadline is recorded in `deadline` for the panic fallback.
fn run(environment: Environment, deadline: &OnceLock<Instant>) -> (Value, Option<Watchdog>) {
    if environment.args != ARGUMENTS {
        return (
            report(None, Err(Failure::Configuration), Cleanup::NotApplicable),
            None,
        );
    }
    let validated = match load(&environment) {
        Ok(validated) => validated,
        Err(error) => return (report(None, Err(error), Cleanup::NotApplicable), None),
    };
    let _ = deadline.set(validated.deadline);
    let watchdog = match Watchdog::start(validated.deadline) {
        Ok(watchdog) => watchdog,
        Err(error) => {
            return (
                report(Some(&validated), Err(error), Cleanup::NotApplicable),
                None,
            )
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return (
                report(
                    Some(&validated),
                    Err(Failure::Configuration),
                    Cleanup::NotApplicable,
                ),
                Some(watchdog),
            )
        }
    };
    let settings = Settings::new(CREDENTIALS.into(), REGISTRY.into(), CA.into(), HOST.into());
    let (outcome, cleanup) = runtime.block_on(supervise(
        settings,
        validated.deadline,
        production_cases,
        || validate_files(&validated, &Paths::fixed(), UID),
    ));
    runtime.shutdown_timeout(Duration::ZERO);
    (report(Some(&validated), outcome, cleanup), Some(watchdog))
}

/// Installs the silent panic hook, then runs `body`. A panic escaping `body`
/// (outside the case task) becomes `entry_panic` with cleanup unconfirmed.
/// Unwinding drops the body's watchdog, so when `body` had recorded the
/// original deadline a new watchdog is armed at it before returning; it
/// stays armed until the result line is written.
fn guarded(
    body: impl FnOnce(&OnceLock<Instant>) -> (Value, Option<Watchdog>),
) -> (Value, Option<Watchdog>) {
    // No assertion or driver panic payload may reach the harness output.
    std::panic::set_hook(Box::new(|_| {}));
    let deadline = OnceLock::new();
    catch_unwind(AssertUnwindSafe(|| body(&deadline))).unwrap_or_else(|_| {
        (
            report(None, Err(Failure::EntryPanic), Cleanup::Unconfirmed),
            deadline
                .get()
                .and_then(|deadline| Watchdog::start(*deadline).ok()),
        )
    })
}

/// Writes the single bounded result line; returns whether it passed.
fn emit(result: Value, watchdog: Option<Watchdog>, out: &mut impl Write) -> bool {
    let (passed, encoded) = match serde_json::to_string(&result) {
        Ok(encoded) if encoded.len() <= RESULT_LIMIT => (result["passed"] == true, encoded),
        // Only the report's own boolean cleanup state survives; anything
        // else falls back to applicable and unconfirmed.
        _ => (
            false,
            json!({"schema_version":1,"scope":SCOPE,"passed":false,"error":"report_refused",
                "checks":[],"cleanup_applicable":result["cleanup_applicable"].as_bool().unwrap_or(true),
                "cleanup_confirmed":result["cleanup_confirmed"].as_bool().unwrap_or(false)})
            .to_string(),
        ),
    };
    // libtest has already written "test <name> ... " without a newline; the
    // leading newline puts the result at the start of its own line.
    let written = write!(out, "\n{RESULT_PREFIX} {encoded}\n").and_then(|_| out.flush());
    // The watchdog stays armed until the result line is written.
    drop(watchdog);
    passed && written.is_ok()
}

#[test]
#[ignore = "operator-only exact selector, fixed arming and private immutable run manifest required"]
fn managed_readonly_entry() -> std::result::Result<(), &'static str> {
    // Paired clock samples precede every file read (see the module docs).
    let environment = Environment {
        args: std::env::args().skip(1).collect(),
        euid: unsafe { libc::geteuid() },
        arm: std::env::var(ARM).ok(),
        now: SystemTime::now(),
        monotonic: Instant::now(),
    };
    let (result, watchdog) = guarded(|deadline| run(environment, deadline));
    if emit(result, watchdog, &mut std::io::stdout().lock()) {
        Ok(())
    } else {
        Err("managed qualification refused")
    }
}

mod tests;
