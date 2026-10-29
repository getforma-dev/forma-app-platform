//! APP-010 definition campaign. Isolated test executable only: local Docker,
//! OpenSSL, a throwaway CA and the locked Python environments, exactly as the
//! APP-008 native and APP-073 admission campaigns. Definitions are admitted by
//! `crate::admission`, drafted and saved by genui_workspace and published by
//! genui_control through the storage module over verified TLS. The operator
//! command backfills the audit heads and verifies every chain. Fixture-only
//! operator SQL (the local superuser acting as genui_owner under explicit
//! scope, as `tenant_seed` and the D7.19 operator channel do) writes what no
//! application role may: the app shell, permission rows and the Northstar
//! expenses source revision (APP-077 and the import tickets own their product
//! paths). It also builds the Northstar Spend Review S05 fixture (plan Task 6)
//! that APP-013's command campaign reuses through [`spend_review_s05`].
use super::{
    admission_qualification::operator,
    authorization::Transaction,
    pool::TestHooks,
    qualification::{fixture_python, Fixture},
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL},
    *,
};
use crate::admission::{self, digest, AdmissionError, AdmittedDefinition};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant as WallInstant},
};
use tokio::time::timeout;

const ISSUER: &str = "app008-fixture-only";
/// The Spend Review author, seeded active with an entitlement in both
/// organizations (tenant_seed); owner of the Northstar Spend Review app.
pub(super) const AUTHOR: &str = "00000000-0000-4000-8000-0000000000ca";
/// The Spend Review app identity; in Northstar it exists only once this
/// campaign creates it. The seeded canary rows keep the same app id.
pub(super) const SPEND_APP: &str = "00000000-0000-4000-8000-0000000000c9";
/// tenant_seed.OWNER_PUBLISH_POLICY, version 1, in both organizations.
pub(super) const POLICY: &str = "00000000-0000-4000-8000-0000000000e0";
/// Northstar's own expenses source revision (operator stand-in).
pub(super) const EXPENSES: &str = "00000000-0000-4000-8000-0000000000e2";
pub(super) const EXPENSES_REVISION: &str = "00000000-0000-4000-8000-0000000000e3";
/// The canary organization's seeded Spend Review source revision.
const CANARY_SOURCE: &str = "00000000-0000-4000-8000-0000000000d2";
const CANARY_REVISION: &str = "00000000-0000-4000-8000-0000000000d3";
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
const FIXTURE: &str = "contracts/fixtures/impl-2026-10-28.1/spend-review.json";

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn bundle() -> Value {
    serde_json::from_slice(&std::fs::read(repository().join(FIXTURE)).unwrap()).unwrap()
}

pub(super) fn request(
    stores: &Stores,
    organization: &str,
    principal: &str,
    epoch: i64,
) -> TrustedRequest {
    TrustedRequest {
        organization: organization.into(),
        principal: principal.into(),
        epoch,
        issuer: ISSUER.into(),
        provider_tenant: format!("ten_{organization}"),
        provider_subject: format!("usr_{principal}"),
        registry_sha256: stores.registry_identity().sha256.clone(),
    }
}

/// Operator-channel stand-in: genui_owner under explicit scope, one
/// transaction (as tenant_seed writes fixture authority).
fn owner_sql(fixture: &Fixture, organization: &str, statements: &str) {
    fixture.sql(
        &format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{organization}',true); {statements} COMMIT;"),
        "genui",
    );
}
fn hexed(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// Every row this campaign's writes can touch, as the superuser counts them.
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in [
        "apps",
        "definition_revisions",
        "definition_inputs",
        "definition_record_schemas",
        "publications",
        "publication_inputs",
        "publication_record_schemas",
        "saved_work",
        "record_collections",
        "work_bindings",
        "work_source_dependencies",
        "audit_events",
    ] {
        counts.push(scalar(fixture, &format!("SELECT count(*) FROM genui.{table}")).await);
    }
    counts
}

/// The Spend Review definition of edition impl-2026-10-28.1 (schema version
/// 1, the Phase 1 edition), its envelope re-scoped to `organization`; the
/// content and therefore its digest are unchanged.
pub(super) fn spend_review_definition(organization: &str) -> Value {
    let mut definition = bundle()["definition"].clone();
    definition["organization_id"] = json!(organization);
    definition
}
fn admit(definition: &Value) -> std::result::Result<AdmittedDefinition, AdmissionError> {
    admission::admit_definition(&admission::encode(definition).unwrap())
}
/// The Northstar expenses source revision: the canary's synthetic APP-002
/// revision document under Northstar's own identifiers (placeholder digests,
/// never imported data).
fn expenses_revision() -> Value {
    let mut source = bundle()["sources"][0].clone();
    source["organization_id"] = json!(ORG);
    source["source_id"] = json!(EXPENSES);
    source["revision_id"] = json!(EXPENSES_REVISION);
    source
}

/// The S05 Spend Review in Northstar, created through the admitted path.
pub(super) struct SpendReview {
    pub revision: String,
    pub publication: String,
    pub work: String,
    pub collection: String,
    pub definition_digest: String,
}

/// Plan Task 6: create the Spend Review app, publication and saved work in
/// the Northstar organization through the admitted path, as the S05 fixture
/// for APP-013. The operator stand-in writes the app shell, the author's app
/// and source grants and Northstar's expenses revision before, and the work
/// grants after (permission rows are operator-only, ADR-007 D7.3).
pub(super) async fn spend_review_s05(fixture: &Fixture, stores: &Stores) -> SpendReview {
    let source = expenses_revision();
    let contract = admission::encode(&source["input_contract"]).unwrap();
    owner_sql(
        fixture,
        ORG,
        &format!(
            "INSERT INTO genui.apps VALUES ('{ORG}','{SPEND_APP}','{SPACE}','{AUTHOR}');
             INSERT INTO genui.app_permissions VALUES ('{ORG}','{AUTHOR}','{SPEND_APP}','discover'),('{ORG}','{AUTHOR}','{SPEND_APP}','run'),('{ORG}','{AUTHOR}','{SPEND_APP}','edit_definition');
             INSERT INTO genui.sources VALUES ('{ORG}','{EXPENSES}');
             INSERT INTO genui.source_revisions VALUES ('{ORG}','{EXPENSES}','{EXPENSES_REVISION}','{}','{}','{}',NULL,'\\x{}'::bytea);
             INSERT INTO genui.source_permissions VALUES ('{ORG}','{AUTHOR}','{EXPENSES}','source_read');",
            source["content_digest"].as_str().unwrap(),
            source["normalized_digest"].as_str().unwrap(),
            digest::digest(digest::INPUT_CONTRACT, &contract),
            hexed(&admission::encode(&source).unwrap()),
        ),
    );
    let author = request(stores, ORG, AUTHOR, 1);
    let admitted = admit(&spend_review_definition(ORG)).expect("edition-1 Spend Review admits");
    let revision = stores
        .workspace
        .create_definition_revision(&author, SPEND_APP, &admitted)
        .await
        .unwrap();
    assert_eq!(revision.digest, admitted.digest);
    let PublishResult::Published(publication) = stores
        .control
        .publish(&author, SPEND_APP, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let work = stores
        .workspace
        .create_work(
            &author,
            SPEND_APP,
            &publication.publication_id,
            "Spend Review S05 fixture (synthetic)",
            &[
                WorkBinding::Source {
                    input: "expenses".into(),
                    source: EXPENSES.into(),
                    revision: EXPENSES_REVISION.into(),
                },
                WorkBinding::Records {
                    input: "decisions".into(),
                    schema: "decision".into(),
                },
            ],
        )
        .await
        .unwrap();
    let w = &work.work;
    owner_sql(
        fixture,
        ORG,
        &format!(
            "INSERT INTO genui.work_permissions VALUES ('{ORG}','{AUTHOR}','{SPEND_APP}','{w}','record_read'),('{ORG}','{AUTHOR}','{SPEND_APP}','{w}','record_write'),('{ORG}','{AUTHOR}','{SPEND_APP}','{w}','share_work');"
        ),
    );
    let collection = fixture
        .operator()
        .await
        .client
        .query_one(
            "SELECT collection_id::text FROM genui.record_collections WHERE organization_id=$1::text::uuid AND work_id=$2::text::uuid AND schema_id='decision'",
            &[&ORG, &w],
        )
        .await
        .unwrap()
        .get(0);
    SpendReview {
        revision: revision.revision,
        publication: publication.publication_id,
        work: work.work,
        collection,
        definition_digest: revision.digest,
    }
}

/// contracts/validate.py over the stored documents: each validates alone, and
/// the edition-1 Spend Review bundle (its receipt, command and outcome
/// re-identified onto the stored ids) passes `validate_bundle`.
const CROSS_CHECK: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
from validate import validate, validate_bundle, digest, canonical, payload_digest
d = json.load(sys.stdin)
for name in ("definition", "publication", "work", "source"):
    validate(d[name])
fixture = json.load(open(sys.argv[2]))
ids = d["ids"]
def replace(value):
    if isinstance(value, dict): return {k: replace(v) for k, v in value.items()}
    if isinstance(value, list): return [replace(v) for v in value]
    return ids.get(value, value) if isinstance(value, str) else value
bundle = {"context": {"organization_id": d["definition"]["organization_id"], "principal_id": d["author"]},
          "definition": d["definition"], "publication": d["publication"], "work": d["work"], "sources": [d["source"]],
          "receipt": replace(fixture["receipt"]), "command": replace(fixture["command"]), "outcome": replace(fixture["outcome"])}
bundle["receipt"]["digest"] = digest("query-receipt", {k: v for k, v in bundle["receipt"].items() if k != "digest"})
bundle["outcome"]["payload_digest"] = payload_digest(bundle["command"])
validate_bundle(bundle)
print(json.dumps({"definition_digest": digest("app-definition", d["definition"]["content"]),
                  "inputs": {i["id"]: digest("input-contract", i) for i in d["definition"]["content"]["inputs"]},
                  "record_schemas": {s["id"]: digest("record-schema", s) for s in d["definition"]["content"]["record_schemas"]},
                  "canonical": {k: canonical(d[k]).hex() for k in ("definition", "publication", "work", "source")},
                  "bundle": "validated"}))
"#;
fn python(input: &Value) -> Value {
    let repo = repository();
    let mut child = Command::new(fixture_python())
        .current_dir(&repo)
        .args([
            "-c",
            CROSS_CHECK,
            repo.join("contracts").to_str().unwrap(),
            repo.join(FIXTURE).to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("locked Python available");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(input).unwrap())
        .unwrap();
    let end = WallInstant::now() + Duration::from_secs(45);
    while child.try_wait().unwrap().is_none() {
        if WallInstant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("contract cross-check deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "contracts/validate.py refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
async fn chain_events(fixture: &Fixture, writer: &str) -> i64 {
    scalar(
        fixture,
        &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='{writer}'"),
    )
    .await
}
async fn paused(hooks: &TestHooks) {
    timeout(Duration::from_secs(5), hooks.reached.notified())
        .await
        .expect("publication barrier reached");
}
fn published(result: PublishResult) -> Publication {
    match result {
        PublishResult::Published(publication) => publication,
        PublishResult::Stale(current) => panic!("unexpected stale publication: {current:?}"),
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_definition_campaign() {
    std::thread::Builder::new()
        .name("app010-definition-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn campaign() {
    let mut fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();

    // --- Operator preconditions: policy rows (seed) and audit heads (backfill) ---
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publication_policies WHERE policy_id='{POLICY}' AND policy_version=1 AND kind='owner_publish'")).await,
        2
    );
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["backfill", "--organization", org]);
        assert_eq!(out["created"].as_array().unwrap().len(), 5);
    }
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND app_id='{SPEND_APP}'")).await,
        0
    );
    check(
        &mut checks,
        "operator seed gives both organizations their owner_publish policy row and the operator backfill their five genesis heads; Spend Review exists only in the canary organization",
    );

    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    stores.readiness().await.unwrap();
    let author = request(&stores, ORG, AUTHOR, 1);

    // --- Whole-output admission refuses before storage, with a coded reason ---
    let before = footprint(&fixture).await;
    let mut refusals = Vec::new();
    for (code, pointer, value) in [
        (
            "client_grant",
            "/content/permissions",
            json!(["admin_policy"]),
        ),
        ("unknown_field", "/content/theme", json!("dark")),
        (
            "executable_content",
            "/content/views/0/script",
            json!("<script>alert(1)</script>"),
        ),
        (
            "unregistered_operator",
            "/content/views/0/query/operations/0/type",
            json!("pivot"),
        ),
        (
            "unregistered_renderer",
            "/content/views/0/kind",
            json!("gauge"),
        ),
    ] {
        let mut definition = spend_review_definition(ORG);
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let target = definition.pointer_mut(parent).unwrap();
        match target {
            Value::Object(map) => {
                map.insert(key.into(), value);
            }
            _ => unreachable!(),
        }
        let content = admission::encode(&definition["content"]).unwrap();
        definition["digest"] = json!(digest::digest(digest::APP_DEFINITION, &content));
        let error = admit(&definition).unwrap_err();
        assert_eq!(error.code(), code, "{pointer}");
        refusals.push(json!({"mutation": pointer, "code": error.code()}));
    }
    let mut views = spend_review_definition(ORG);
    let view = views["content"]["views"][0].clone();
    let list = views["content"]["views"].as_array_mut().unwrap();
    while list.len() < 25 {
        let mut extra = view.clone();
        extra["id"] = json!(format!("extra_{}", list.len()));
        list.push(extra);
    }
    let content = admission::encode(&views["content"]).unwrap();
    views["digest"] = json!(digest::digest(digest::APP_DEFINITION, &content));
    assert_eq!(admit(&views).unwrap_err(), AdmissionError::TooManyViews(25));
    refusals.push(json!({"mutation": "25 views", "code": "too_many_views"}));
    // A caller-built AdmittedDefinition (public fields) is re-admitted and refused:
    // content carrying a client grant under its own digest, and a digest that
    // does not name the content.
    let genuine = admit(&spend_review_definition(ORG)).unwrap();
    let mut forged = genuine.clone();
    let mut patched = admission::decode(&forged.canonical).unwrap();
    patched["permissions"] = json!(["review_publish"]);
    forged.canonical = admission::encode(&patched).unwrap();
    forged.digest = digest::digest(digest::APP_DEFINITION, &forged.canonical);
    let mut misnamed = genuine.clone();
    misnamed.digest = format!("sha256:{}", "0".repeat(64));
    // Edition 2 admits, but Phase 1 stores edition 1 only (0002).
    let current: Value = serde_json::from_slice(
        &std::fs::read(repository().join("contracts/fixtures/spend-review.json")).unwrap(),
    )
    .unwrap();
    let edition_two = admit(&current["definition"]).unwrap();
    assert_eq!(edition_two.schema_version, 2);
    // The Northstar author holds edit_definition on the Northstar app, so each
    // refusal is the re-admission's, not the authorizer's.
    let editor = request(&stores, ORG, PRINCIPAL, 1);
    for (label, candidate) in [
        ("forged", &forged),
        ("misnamed", &misnamed),
        ("edition 2", &edition_two),
    ] {
        assert_eq!(
            stores
                .workspace
                .create_definition_revision(&editor, APP, candidate)
                .await,
            Err(Error::Denied),
            "{label}"
        );
    }
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "client grants, unknown fields, executable strings, unregistered operators and renderers and 25 views fail whole-output admission with their coded reason; a caller-forged admitted definition and an edition-2 definition are refused by storage; nothing is written",
    );

    // --- The S05 fixture through the admitted path ---
    let s05 = spend_review_s05(&fixture, &stores).await;
    assert_eq!(
        s05.definition_digest,
        bundle()["definition"]["digest"].as_str().unwrap()
    );
    check(
        &mut checks,
        "Northstar Spend Review created through the admitted path: workspace drafts the admitted revision, control publishes it under owner_publish, workspace saves work binding Northstar's own expenses revision and a decision collection (S05 fixture)",
    );

    // --- Read back: definition, publication, saved work ---
    let definition = stores
        .workspace
        .read_definition(&author, SPEND_APP, &s05.revision)
        .await
        .unwrap();
    assert_eq!(definition.digest, genuine.digest);
    assert_eq!(definition.canonical, genuine.canonical);
    let publication = stores
        .workspace
        .read_publication_detail(&author, SPEND_APP, &s05.publication)
        .await
        .unwrap();
    let work = stores
        .workspace
        .read_work(&author, SPEND_APP, &s05.work)
        .await
        .unwrap();
    assert_eq!(work.version, 1);
    assert_eq!(work.publication, s05.publication);
    assert_eq!(work.definition_digest, genuine.digest);
    let envelope: Value = serde_json::from_slice(&publication.envelope).unwrap();
    let saved: Value = serde_json::from_slice(&work.canonical).unwrap();
    assert_eq!(publication.admission_digest, genuine.digest);
    assert_eq!(envelope["policy"]["kind"], "owner_publish");
    assert_eq!(envelope["author_id"], AUTHOR);
    assert_eq!(envelope["approver_id"], AUTHOR);
    check(
        &mut checks,
        "stored revision re-admits to the same digest and bytes; publication and saved work read back as canonical documents pinned to that digest",
    );

    // --- C02: the reusable publication carries contracts, never bindings ---
    let text = String::from_utf8(publication.envelope.clone()).unwrap();
    for private in [
        EXPENSES,
        EXPENSES_REVISION,
        &s05.work,
        &s05.collection,
        "source_id",
        "source_revision_id",
        "collection_id",
        "bindings",
    ] {
        assert!(!text.contains(private), "{private}");
    }
    let content = admission::decode(&definition.canonical).unwrap();
    let expected: Vec<Value> = content["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| {
            json!({"input_id": input["id"], "digest": digest::digest(digest::INPUT_CONTRACT, &admission::encode(input).unwrap())})
        })
        .collect();
    assert_eq!(envelope["input_contract_digests"], json!(expected));
    assert_eq!(publication.inputs, definition.inputs);
    assert_eq!(publication.record_schemas, definition.record_schemas);
    for input in &publication.inputs {
        assert_eq!(
            input.digest,
            digest::digest(digest::INPUT_CONTRACT, &input.canonical)
        );
        let contract = admission::decode(&input.canonical).unwrap();
        assert!(contract.get("source_id").is_none() && contract.get("fields").is_some());
    }
    let bindings = saved["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[1]["source_id"], EXPENSES);
    assert_eq!(
        bindings[0]["collection_id"].as_str(),
        Some(&s05.collection[..])
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publication_inputs WHERE organization_id='{ORG}' AND publication_id='{}'", s05.publication)).await,
        2
    );
    check(
        &mut checks,
        "the publication contains the typed input contracts and record schema (digests over their canonical bytes) and no source, revision, collection or work identity; concrete bindings exist only in the saved work",
    );

    // --- Contract cross-check: Rust and contracts/validate.py agree ---
    let definition_document = json!({
        "kind": "app_definition", "schema_version": 1, "organization_id": ORG, "app_id": SPEND_APP,
        "revision_id": s05.revision, "content": content, "digest": definition.digest,
    });
    let source = expenses_revision();
    let fixture_bundle = bundle();
    let python_result = python(&json!({
        "definition": definition_document, "publication": envelope, "work": saved, "source": source,
        "author": AUTHOR,
        "ids": {
            fixture_bundle["context"]["organization_id"].as_str().unwrap(): ORG,
            fixture_bundle["definition"]["revision_id"].as_str().unwrap(): s05.revision,
            fixture_bundle["publication"]["publication_id"].as_str().unwrap(): s05.publication,
            fixture_bundle["work"]["work_id"].as_str().unwrap(): s05.work,
            fixture_bundle["work"]["record_collections"][0]["id"].as_str().unwrap(): s05.collection,
            CANARY_SOURCE: EXPENSES,
            CANARY_REVISION: EXPENSES_REVISION,
        },
    }));
    assert_eq!(python_result["bundle"], "validated");
    assert_eq!(python_result["definition_digest"], json!(definition.digest));
    for input in &definition.inputs {
        assert_eq!(python_result["inputs"][&input.input], json!(input.digest));
    }
    for schema in &definition.record_schemas {
        assert_eq!(
            python_result["record_schemas"][&schema.schema],
            json!(schema.digest)
        );
    }
    assert_eq!(
        python_result["canonical"]["publication"],
        json!(hexed(&publication.envelope))
    );
    assert_eq!(
        python_result["canonical"]["work"],
        json!(hexed(&work.canonical))
    );
    assert_eq!(
        python_result["canonical"]["definition"],
        json!(hexed(&admission::encode(&definition_document).unwrap()))
    );
    check(
        &mut checks,
        "contracts/validate.py validates the stored definition, publication, saved work and source revision, agrees on every definition, input-contract and record-schema digest and on the canonical bytes, and accepts the re-identified Spend Review bundle (validate_bundle)",
    );

    // --- Scope and authority negatives; refusals write nothing ---
    let before = footprint(&fixture).await;
    let foreign = stores
        .workspace
        .create_work(
            &author,
            SPEND_APP,
            &s05.publication,
            "",
            &[
                WorkBinding::Source {
                    input: "expenses".into(),
                    source: CANARY_SOURCE.into(),
                    revision: CANARY_REVISION.into(),
                },
                WorkBinding::Records {
                    input: "decisions".into(),
                    schema: "decision".into(),
                },
            ],
        )
        .await;
    assert_eq!(foreign, Err(Error::Denied));
    let canary_author = request(&stores, OTHER, AUTHOR, 1);
    assert_eq!(
        stores
            .workspace
            .create_work(
                &canary_author,
                SPEND_APP,
                &s05.publication,
                "",
                &[WorkBinding::Source {
                    input: "expenses".into(),
                    source: CANARY_SOURCE.into(),
                    revision: CANARY_REVISION.into(),
                }],
            )
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .control
            .publish(&canary_author, SPEND_APP, &s05.revision, POLICY)
            .await,
        Err(Error::Denied)
    );
    let incomplete = stores
        .workspace
        .create_work(
            &author,
            SPEND_APP,
            &s05.publication,
            "",
            &[WorkBinding::Source {
                input: "expenses".into(),
                source: EXPENSES.into(),
                revision: EXPENSES_REVISION.into(),
            }],
        )
        .await;
    assert_eq!(incomplete, Err(Error::Denied));
    let mismatched = stores
        .workspace
        .create_work(
            &author,
            SPEND_APP,
            &s05.publication,
            "",
            &[
                WorkBinding::Records {
                    input: "expenses".into(),
                    schema: "decision".into(),
                },
                WorkBinding::Records {
                    input: "decisions".into(),
                    schema: "decision".into(),
                },
            ],
        )
        .await;
    assert_eq!(mismatched, Err(Error::Denied));
    // A member without edit_definition (the Northstar author) cannot draft
    // Spend Review; under owner_publish a non-owner cannot publish even with
    // edit_definition; a revoked member is denied everything.
    let other_member = request(&stores, ORG, PRINCIPAL, 1);
    assert_eq!(
        stores
            .workspace
            .create_definition_revision(&other_member, SPEND_APP, &genuine)
            .await,
        Err(Error::Denied)
    );
    owner_sql(
        &fixture,
        ORG,
        &format!("INSERT INTO genui.app_permissions VALUES ('{ORG}','{PRINCIPAL}','{SPEND_APP}','edit_definition');"),
    );
    assert_eq!(
        stores
            .control
            .publish(&other_member, SPEND_APP, &s05.revision, POLICY)
            .await,
        Err(Error::Denied)
    );
    let revoked = request(&stores, ORG, REVOKED, 2);
    assert_eq!(
        stores
            .workspace
            .read_work(&revoked, SPEND_APP, &s05.work)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .read_work(&other_member, SPEND_APP, &s05.work)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "binding the canary's Spend Review source from Northstar, publishing or saving Northstar's revision under canary scope, leaving an input unbound, binding a source input as records, drafting without edit_definition, publishing as a non-owner under owner_publish and reading as a revoked or ungranted member are all Denied and write nothing",
    );

    // --- Role separation at the database (D7.3) ---
    let pub_sql = format!("INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope) VALUES ('{ORG}','{SPEND_APP}',gen_random_uuid(),'{rev}','{d}','{POLICY}',1,'owner_publish','{AUTHOR}','{AUTHOR}',1,1,1,'{d}',now(),'\\x7b7d')", rev = s05.revision, d = genuine.digest);
    let work_sql = format!("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context) VALUES ('{ORG}','{SPEND_APP}',gen_random_uuid(),'{}',1,'forged')", s05.publication);
    for (pool, statement) in [
        (&stores.workspace.inner, pub_sql.clone()),
        (&stores.data.inner, pub_sql),
        (&stores.control.inner, work_sql.clone()),
        (&stores.data.inner, work_sql),
        (
            &stores.workspace.inner,
            format!("UPDATE genui.saved_work SET version=2 WHERE organization_id='{ORG}'"),
        ),
        (
            &stores.control.inner,
            format!("UPDATE genui.audit_heads SET chain_sequence=chain_sequence+1 WHERE organization_id='{ORG}' AND writer_role='workspace'"),
        ),
    ] {
        let transaction = Transaction::begin(pool, &author).await.unwrap();
        let outcome = transaction.lease.client().execute(&statement, &[]).await;
        match outcome {
            Err(error) => assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
                "{statement}"
            ),
            // The RESTRICTIVE head policy makes another chain's head invisible.
            Ok(rows) => assert!(statement.starts_with("UPDATE genui.audit_heads") && rows == 0),
        }
        transaction.finish(false).await.unwrap();
    }
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "workspace and data cannot write publications, control and data cannot write saved work, workspace cannot bump a saved work version, control cannot advance the workspace chain (42501 or no visible row); nothing written",
    );

    // --- D7.6: one 40001 retry, then Stale ---
    let control_before = chain_events(&fixture, "control").await;
    let failures =
        || super::definitions::SERIALIZATION_FAILURES.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(failures(), 0);
    hooks.arm("publish");
    let racer = {
        let stores = stores.clone();
        let revision = s05.revision.clone();
        tokio::spawn(async move {
            let author = request(&stores, ORG, AUTHOR, 1);
            stores
                .control
                .publish(&author, SPEND_APP, &revision, POLICY)
                .await
        })
    };
    paused(&hooks).await;
    let winner = published(
        stores
            .control
            .publish(&author, SPEND_APP, &s05.revision, POLICY)
            .await
            .unwrap(),
    );
    hooks.release.notify_one();
    let retried = published(racer.await.unwrap().unwrap());
    assert_ne!(retried.publication_id, winner.publication_id);
    assert_eq!(failures(), 1);
    assert_eq!(chain_events(&fixture, "control").await, control_before + 2);
    check(
        &mut checks,
        "a SERIALIZABLE publication whose control-chain head was advanced after its snapshot fails with 40001, rolls back whole and succeeds on its single retry in a new transaction; both publications and exactly two control events exist",
    );
    hooks.arm("publish");
    let racer = {
        let stores = stores.clone();
        let revision = s05.revision.clone();
        tokio::spawn(async move {
            let author = request(&stores, ORG, AUTHOR, 1);
            stores
                .control
                .publish(&author, SPEND_APP, &revision, POLICY)
                .await
        })
    };
    paused(&hooks).await;
    published(
        stores
            .control
            .publish(&author, SPEND_APP, &s05.revision, POLICY)
            .await
            .unwrap(),
    );
    hooks.arm("publish");
    hooks.release.notify_one();
    paused(&hooks).await;
    // Whole-second published_at: the newest publication is unambiguous.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let newest = published(
        stores
            .control
            .publish(&author, SPEND_APP, &s05.revision, POLICY)
            .await
            .unwrap(),
    );
    hooks.release.notify_one();
    let stale = racer.await.unwrap().unwrap();
    assert_eq!(
        stale,
        PublishResult::Stale(CurrentRevision {
            publication: Some(newest.publication_id.clone()),
            revision: Some(s05.revision.clone()),
            definition_digest: Some(genuine.digest.clone()),
        })
    );
    assert_eq!(failures(), 3);
    assert_eq!(chain_events(&fixture, "control").await, control_before + 4);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{SPEND_APP}'")).await,
        5
    );
    check(
        &mut checks,
        "a publication that loses to a concurrent writer on both attempts returns PublishResult::Stale naming the newest publication (412); neither attempt left a publication or event",
    );

    // --- Restart: a new store reloads identical bytes and digests ---
    let publication_again = stores
        .workspace
        .read_publication_detail(&author, SPEND_APP, &s05.publication)
        .await
        .unwrap();
    assert_eq!(publication_again, publication);
    let stores = Arc::try_unwrap(stores).unwrap_or_else(|_| panic!("no outstanding store"));
    stores.close().await.unwrap();
    assert_eq!(super::pool::slots().0, 0);
    drop(stores);
    let restarted = Stores::new(fixture.settings()).unwrap();
    restarted.readiness().await.unwrap();
    let author = request(&restarted, ORG, AUTHOR, 1);
    assert_eq!(
        restarted
            .workspace
            .read_definition(&author, SPEND_APP, &s05.revision)
            .await
            .unwrap(),
        definition
    );
    assert_eq!(
        restarted
            .workspace
            .read_publication_detail(&author, SPEND_APP, &s05.publication)
            .await
            .unwrap(),
        publication
    );
    assert_eq!(
        restarted
            .workspace
            .read_work(&author, SPEND_APP, &s05.work)
            .await
            .unwrap(),
        work
    );
    check(
        &mut checks,
        "after closing every pool and building new stores, the admitted definition, its publication and the saved work reload with identical canonical bytes and digests",
    );

    // --- D7.8: both application chains verify from genesis ---
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    // definition.drafted, work.saved and two work.bound for the S05 fixture.
    assert_eq!(out["chains"]["workspace"]["events"].as_i64(), Some(4));
    assert_eq!(out["chains"]["control"]["events"].as_i64(), Some(5));
    let kinds: Vec<String> = fixture
        .operator()
        .await
        .client
        .query(
            &format!("SELECT kind FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role IN ('workspace','control') ORDER BY writer_role,chain_sequence"),
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(
        kinds,
        [
            "publication.published",
            "publication.published",
            "publication.published",
            "publication.published",
            "publication.published",
            "definition.drafted",
            "work.saved",
            "work.bound",
            "work.bound",
        ]
    );
    let out = operator(&fixture, &["verify", "--organization", OTHER]);
    assert_eq!(out["chains"]["workspace"]["events"].as_i64(), Some(0));
    check(
        &mut checks,
        "the operator recomputes the workspace (definition.drafted, work.saved, work.bound) and control (publication.published) chains from their genesis heads to the heads; the canary chains are untouched",
    );

    restarted.close().await.unwrap();
    super::qualification::docker(
        &["rm", "--force", "--volumes", &fixture.name],
        b"",
        "remove isolated database",
    );
    fixture.removed = true;
    std::fs::remove_dir_all(&fixture.directory.0).unwrap();
    check(
        &mut checks,
        "local database and private fixture files removed",
    );
    if let Some(path) = std::env::var_os("GENUI_DEFINITION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "scope": "APP-010 isolated native PostgreSQL TLS definition, publication and saved-work qualification",
            "managed_acceptance": false,
            "server_version": fixture.server_version,
            "s05": {"organization_id": ORG, "app_id": SPEND_APP, "definition_digest": genuine.digest,
                    "policy": POLICY, "expenses_source": EXPENSES, "expenses_revision": EXPENSES_REVISION},
            "admission_refusals": refusals,
            "checks": checks,
            "passed": true,
            "limits": [
                "Synthetic local password authentication is not RDS IAM.",
                "Identities are synthetic registry-bound fixtures; no session, route or browser flow is exercised (APP-009/012/013).",
                "The app shell, permission rows and the Northstar expenses source revision are written by fixture-only operator SQL; their product paths are APP-077 and the import tickets'.",
                "Source digests are the APP-002 synthetic placeholders; no source bytes were imported.",
                "The database itself is not restarted (tmpfs fixture); the restart is of every application pool and store.",
                "The 40001 path is exercised at a statement (the control head lock); a 40001 at COMMIT is classified by the same code path but not provoked here."
            ]
        })).unwrap()).unwrap();
    }
}
