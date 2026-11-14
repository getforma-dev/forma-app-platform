//! APP-093 composer campaign. Isolated test executable only: local Docker,
//! OpenSSL, a throwaway CA and the locked Python environments, exactly as the
//! APP-010 definition campaign. A scripted model (nothing here calls the real
//! endpoint) drives `composer::routes::compose_for` over real rows: the
//! member's context names only what the member owns, may read and may run;
//! a revoked member and a principal with no row compose nothing (the uniform
//! refusal) and no shell is written for them; an active member who owns no
//! app receives one authoring shell (APP-145); a foreign
//! organization's library is never offered; the disabled composer answers
//! neutrally; a confident router choice instantiates the library app as a
//! new saved work labeled `retrieved` under the member's own `source_read`;
//! a generated definition is admitted, drafted on the owned app,
//! owner-published and saved, labeled `generated`, with every audit event;
//! hostile outputs are refused by admission on every attempt and leave no
//! row; the adapted patch is re-admitted whole and published; a model
//! failure writes nothing.
use super::{
    admission_qualification::operator,
    definition_qualification::{request, AUTHOR, POLICY, SPEND_APP},
    qualification::Fixture,
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL, WORK},
    *,
};
use crate::composer::{
    model::{Completion, Model, ModelError, ModelReply, ModelRequest},
    plan, prompt,
    routes::{compose_for, Composed, Refused},
    Composer, Origin,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

const NORTHSTAR: &str =
    include_str!("../../../contracts/fixtures/impl-2026-10-28.1/northstar.json");
/// tenant_seed's revoked member. Epoch 2. APP-145 must not create a shell.
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
/// tenant_seed's active member who owns no app and holds no grant.
const BARE: &str = "00000000-0000-4000-8000-00000000006f";
/// No principal row exists for this id.
const NOBODY: &str = "00000000-0000-4000-8000-0000000000ff";
/// An organization with a space, a member and no owner_publish policy.
const EMPTY_ORG: &str = "00000000-0000-4000-8000-000000000145";
const EMPTY_SPACE: &str = "00000000-0000-4000-8000-000000000146";
const EMPTY_MEMBER: &str = "00000000-0000-4000-8000-000000000147";
const EMPTY_ENTITLEMENT: &str = "0000e000-0000-4000-8000-000000000147";

/// A model that answers from a script; every request it saw is kept.
struct Script {
    replies: Mutex<VecDeque<std::result::Result<String, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
}
impl Script {
    fn composer(replies: Vec<std::result::Result<String, ModelError>>) -> (Arc<Self>, Composer) {
        let script = Arc::new(Self {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        });
        let composer = Composer::new(
            Some(script.clone() as Arc<dyn Model>),
            "router-model",
            "generation-model",
        );
        (script, composer)
    }
    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Model for Script {
    fn complete<'a>(&'a self, request: &'a ModelRequest) -> Completion<'a> {
        self.requests.lock().unwrap().push(request.clone());
        let next = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(ModelError::Unavailable));
        Box::pin(async move {
            next.map(|text| ModelReply {
                text,
                model: request.model.clone(),
                input_tokens: None,
                output_tokens: None,
            })
        })
    }
}

fn router_reply(choice: &str, confidence: &str, title: &str) -> String {
    json!({"choice": choice, "confidence": confidence, "reason": "campaign", "title": title})
        .to_string()
}
fn runnable_northstar() -> Value {
    let bundle: Value = serde_json::from_str(NORTHSTAR).unwrap();
    prompt::runnable_example(bundle["definition"]["content"].clone())
}
fn with(mut content: Value, pointer: &str, value: Value) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    content
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(key.into(), value);
    content
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn text(fixture: &Fixture, sql: &str) -> String {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// Every row this campaign's writes can touch in Northstar.
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in [
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
        "apps",
        "app_permissions",
        "work_permissions",
        "source_permissions",
    ] {
        counts.push(
            scalar(
                fixture,
                &format!("SELECT count(*) FROM genui.{table} WHERE organization_id='{ORG}'"),
            )
            .await,
        );
    }
    counts
}
async fn chain_events(fixture: &Fixture, writer: &str, kind: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='{writer}' AND kind='{kind}'")).await
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_composer_campaign() {
    std::thread::Builder::new()
        .name("app093-composer-campaign".into())
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
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["backfill", "--organization", org]);
        assert_eq!(out["created"].as_array().unwrap().len(), 5);
    }
    // begin refuses an organization the closed registry does not bind, before
    // any SQL. The empty organization is inserted later; the stores have to
    // load a registry that already names it.
    fixture.write_registry(&json!({
        "schema_version": 1,
        "registry_version": "00000000-0000-4000-8000-000000000aaa",
        "bindings": [
            {"issuer": "app008-fixture-only", "provider_tenant_id": format!("ten_{ORG}"), "organization_id": ORG},
            {"issuer": "app008-fixture-only", "provider_tenant_id": format!("ten_{OTHER}"), "organization_id": OTHER},
            {"issuer": "app008-fixture-only", "provider_tenant_id": format!("ten_{EMPTY_ORG}"), "organization_id": EMPTY_ORG}
        ]
    }));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let member = request(&stores, ORG, PRINCIPAL, 1);
    let stranger = request(&stores, ORG, AUTHOR, 1);
    let foreign = request(&stores, OTHER, PRINCIPAL, 1);
    let intent = "Which destinations receive the most bytes each minute?";

    // --- The context names only what the member owns, reads and runs ---
    let context = stores.workspace.composer_context(&member).await.unwrap();
    // APP-085 (APP-010 review m3): the member's only owned app is the seeded
    // Northstar app, whose publication went through independent_review. It
    // stays under review, so it is not an authoring app; an explicit compose
    // creates the member's shell instead.
    assert_eq!(context.authoring.as_ref().map(|a| a.app.as_str()), None);
    assert_eq!(context.owner_publish_policy.as_deref(), Some(POLICY));
    assert_eq!(context.sources.len(), 2, "events and inventory revisions");
    assert_eq!(context.library.len(), 1);
    assert_eq!(context.library[0].app, APP);
    assert_eq!(context.library[0].title, "Northstar investigation");
    assert_eq!(context.library[0].inputs.len(), 3);
    assert_eq!(context.library[0].record_schemas.len(), 1);
    assert!(composer_bindings(
        &context.library[0].inputs,
        &context.library[0].record_schemas,
        &context.sources
    )
    .is_some_and(|bindings| bindings.len() == 3));
    let foreign_context = stores.workspace.composer_context(&foreign).await.unwrap();
    assert!(foreign_context
        .library
        .iter()
        .all(|app| app.app != SPEND_APP));
    assert_eq!(
        foreign_context.library.len(),
        1,
        "only the canary's Northstar seed"
    );
    check(
        &mut checks,
        "the composer context names no authoring app while the member's only owned app is under review (APP-085), the owner_publish policy, the two readable source revisions and the one bindable library app; a foreign organization's apps are never offered",
    );

    // --- Revoked and unknown principals compose nothing and get no shell ---
    // An active member who owns no app is the APP-145 case, later. These two
    // stay the uniform refusal, and the refusal must not insert an app row.
    let revoked = request(&stores, ORG, REVOKED, 2);
    let nobody = request(&stores, ORG, NOBODY, 1);
    let untouched = footprint(&fixture).await;
    assert_eq!(
        stores.workspace.composer_context(&revoked).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores.workspace.composer_context(&nobody).await,
        Err(Error::Denied)
    );
    let (script, composer) = Script::composer(vec![Ok(router_reply(APP, "high", "x"))]);
    assert_eq!(
        compose_for(&stores, &composer, &revoked, intent).await,
        Err(Refused::NotFound)
    );
    assert_eq!(
        compose_for(&stores, &composer, &nobody, intent).await,
        Err(Refused::NotFound)
    );
    assert_eq!(
        script.calls(),
        0,
        "no model call for a revoked member or a principal with no row"
    );
    assert_eq!(
        footprint(&fixture).await,
        untouched,
        "a refusal inserts no app, grant or audit row"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE owner_principal_id IN ('{REVOKED}','{NOBODY}')")).await,
        0
    );
    let disabled = Composer::new(None, "r", "g");
    let prepared = footprint(&fixture).await;
    assert_eq!(
        compose_for(&stores, &disabled, &member, intent).await,
        Err(Refused::ComposerUnavailable)
    );
    // APP-085: the member's only owned app is under review, so this explicit
    // compose first prepares the member's authoring shell (APP-145), then
    // refuses for the missing key. Nothing else is written.
    let before = footprint(&fixture).await;
    let mut shell_only = prepared.clone();
    shell_only[10] += 1; // the shell's audit event
    shell_only[11] += 1; // apps
    shell_only[12] += 3; // its discover, edit_definition and run
    assert_eq!(before, shell_only, "only the authoring shell is written");
    let shell = text(&fixture, &format!("SELECT app_id::text FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{PRINCIPAL}' AND app_id<>'{APP}'")).await;
    check(
        &mut checks,
        "a revoked member and a principal with no row get the uniform refusal before any model call and no app row is inserted; without a key the composer answers composer_unavailable and writes nothing but the member's authoring shell (APP-085: the member's owned app is under review)",
    );

    // --- Retrieved: a confident router choice instantiates the library app ---
    let (script, composer) = Script::composer(vec![Ok(router_reply(APP, "high", "ignored"))]);
    let retrieved = compose_for(&stores, &composer, &member, intent)
        .await
        .unwrap();
    assert_eq!(script.calls(), 1, "the router alone; no generation call");
    assert_eq!(retrieved.origin, Origin::Retrieved);
    assert_eq!(retrieved.app, APP);
    assert_ne!(retrieved.work, WORK);
    assert_eq!(retrieved.title, "Northstar investigation");
    let after = footprint(&fixture).await;
    let mut expected = before.clone();
    expected[6] += 1; // saved_work
    expected[7] += 1; // record_collections (decisions)
    expected[8] += 3; // work_bindings
    expected[9] += 2; // work_source_dependencies (events, inventory)
    expected[10] += 4; // work.saved + 3 work.bound
    assert_eq!(after, expected);
    assert_eq!(
        text(&fixture, &format!("SELECT created_by::text FROM genui.saved_work WHERE organization_id='{ORG}' AND work_id='{}'", retrieved.work)).await,
        PRINCIPAL
    );
    assert_eq!(
        text(&fixture, &format!("SELECT context FROM genui.saved_work WHERE organization_id='{ORG}' AND work_id='{}'", retrieved.work)).await,
        intent
    );
    // The creator opens it; a teammate without grants does not (APP-095's
    // premise: sharing is a grant, never a side effect of composing).
    let detail = stores
        .workspace
        .read_work(&member, APP, &retrieved.work)
        .await
        .unwrap();
    assert_eq!(detail.publication, context.library[0].publication);
    assert_eq!(
        stores
            .workspace
            .read_work(&stranger, APP, &retrieved.work)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .work_outline(&member, APP, &retrieved.work)
            .await
            .unwrap()
            .title,
        "Northstar investigation"
    );
    check(
        &mut checks,
        "a high-confidence router choice saves a new work of the library app's publication bound under the member's own source_read, labeled retrieved, with work.saved and work.bound events and no generation call; the creator opens it and an ungranted teammate is denied",
    );

    // --- Generated: admitted, drafted on the member's authoring shell, owner-published, saved ---
    // APP-085: never on the seeded app under review.
    let before = after;
    let reviewed_publications = scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}'")).await;
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(serde_json::to_string(&runnable_northstar()).unwrap()),
    ]);
    let generated = compose_for(&stores, &composer, &member, intent)
        .await
        .unwrap();
    assert_eq!(script.calls(), 2);
    assert_eq!(generated.origin, Origin::Generated);
    assert_eq!(
        generated.app, shell,
        "drafted on the shell, never on the reviewed app"
    );
    assert_eq!(generated.title, "Northstar investigation");
    let after = footprint(&fixture).await;
    let mut expected = before.clone();
    expected[0] += 1; // definition_revisions
    expected[1] += 3; // definition_inputs
    expected[2] += 1; // definition_record_schemas
    expected[3] += 1; // publications
    expected[4] += 3; // publication_inputs
    expected[5] += 1; // publication_record_schemas
    expected[6] += 1; // saved_work
    expected[7] += 1; // record_collections
    expected[8] += 3; // work_bindings
    expected[9] += 2; // work_source_dependencies
    expected[10] += 6; // definition.drafted, publication.published, work.saved, 3 work.bound
    assert_eq!(
        after, expected,
        "no app, permission or grant row is written"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}'")).await,
        reviewed_publications,
        "the app under review gains no publication"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        1
    );
    let detail = stores
        .workspace
        .read_work(&member, &shell, &generated.work)
        .await
        .unwrap();
    assert_ne!(
        detail.publication, context.library[0].publication,
        "pinned to the new publication"
    );
    let newest = text(&fixture, &format!("SELECT publication_id::text FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{shell}' ORDER BY publication_sequence DESC LIMIT 1")).await;
    assert_eq!(detail.publication, newest);
    assert_eq!(
        text(&fixture, &format!("SELECT policy_kind||':'||author_id::text||':'||approver_id::text FROM genui.publications WHERE organization_id='{ORG}' AND publication_id='{newest}'")).await,
        format!("owner_publish:{PRINCIPAL}:{PRINCIPAL}")
    );
    assert_eq!(
        chain_events(&fixture, "workspace", "definition.drafted").await,
        1
    );
    assert_eq!(
        chain_events(&fixture, "control", "publication.published").await,
        1
    );
    assert_eq!(chain_events(&fixture, "workspace", "work.saved").await, 2);
    assert_eq!(chain_events(&fixture, "workspace", "work.bound").await, 6);
    // The generated work runs: its runnable views are the outline's.
    let outline = stores
        .data
        .work_outline(&member, &shell, &generated.work)
        .await
        .unwrap();
    assert_eq!(outline.title, "Northstar investigation");
    assert_eq!(
        outline
            .views
            .iter()
            .map(|view| view.id.as_str())
            .collect::<Vec<_>>(),
        ["total", "timeline"]
    );
    check(
        &mut checks,
        "a generated definition passes admission, is drafted on the member's authoring shell, never on the owned app under review (APP-085), owner-published (author and approver the member) and saved bound to the registered source revisions, labeled generated, with definition.drafted, publication.published, work.saved and work.bound events and no new app, permission or grant row",
    );

    // --- Hostile outputs: refused by admission on every attempt, no row ---
    let before = after;
    let content = runnable_northstar();
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(with(
            content.clone(),
            "/views/0/script",
            json!("<script>alert(1)</script>"),
        )
        .to_string()),
        Ok(with(content.clone(), "/permissions", json!(["admin_policy"])).to_string()),
        Ok(with(
            content.clone(),
            "/views/0/sql",
            json!("DROP TABLE genui.apps; --"),
        )
        .to_string()),
    ]);
    assert_eq!(
        compose_for(&stores, &composer, &member, intent).await,
        Err(Refused::CouldNotCompose("admission_refused"))
    );
    assert_eq!(
        script.calls(),
        4,
        "the router and three generation attempts"
    );
    assert_eq!(
        footprint(&fixture).await,
        before,
        "nothing of a refused output is stored"
    );
    let feedback: Vec<String> = script
        .requests
        .lock()
        .unwrap()
        .iter()
        .skip(1)
        .map(|request| serde_json::from_str::<Value>(&request.user).unwrap()["refusal"].to_string())
        .collect();
    assert_eq!(feedback[0], "null");
    assert!(feedback[1].contains("executable_content"));
    assert!(feedback[2].contains("client_grant"));
    for request in script.requests.lock().unwrap().iter() {
        assert!(!request.user.contains("alert(1)") && !request.user.contains("admin_policy"));
    }
    check(
        &mut checks,
        "script, grant and SQL attempts in model output are refused by admission on every attempt (codes executable_content, client_grant, executable_content), the retries carry the code alone, the member gets the neutral could_not_compose and no row is written",
    );

    // --- Adapted: a typed patch of the library match, re-admitted whole ---
    let (script, composer) =
        Script::composer(vec![Ok(router_reply(APP, "medium", "Traffic review"))]);
    let adapted = compose_for(&stores, &composer, &member, intent)
        .await
        .unwrap();
    assert_eq!(script.calls(), 1, "no generation call");
    assert_eq!(adapted.origin, Origin::Adapted);
    assert_eq!(adapted.title, "Traffic review");
    assert_eq!(
        adapted.app, shell,
        "adapted onto the shell, never onto the reviewed app (APP-085)"
    );
    let after = footprint(&fixture).await;
    assert_eq!(after[3], before[3] + 1, "one more publication");
    assert_eq!(after[6], before[6] + 1, "one more saved work");
    assert_eq!(after[11..], before[11..], "no app, permission or grant row");
    let outline = stores
        .data
        .work_outline(&member, &adapted.app, &adapted.work)
        .await
        .unwrap();
    assert_eq!(outline.title, "Traffic review");
    let (_, origins) = crate::query::canvas::definition_provenance(
        &outline.description,
        &outline.definition_digest,
    );
    assert_eq!(
        origins[0].kind, "adapted",
        "the stored title-only adaptation keeps its origin on reload"
    );
    assert_eq!(
        outline.views.len(),
        2,
        "the seeded Northstar runnable views (total, timeline), patched title"
    );
    check(
        &mut checks,
        "a medium-confidence router choice becomes an adapted definition (title and description patched, re-admitted whole), owner-published on the member's app and saved, still labeled adapted from the persisted definition on reload",
    );

    // --- APP-132: a records-only app (no source input) is generated and saved ---
    // Last of the publishing steps: it becomes the app's newest publication,
    // which the library context offers from here on.
    let before = after;
    let time_off: Value = serde_json::from_str(include_str!(
        "../../../contracts/fixtures/library/time-off.json"
    ))
    .unwrap();
    let records_only = prompt::records_example(time_off["definition"]["content"].clone());
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(records_only.to_string()),
    ]);
    let kept = compose_for(
        &stores,
        &composer,
        &member,
        "Track the team's time-off requests",
    )
    .await
    .unwrap();
    assert_eq!(script.calls(), 2);
    assert_eq!(kept.origin, Origin::Generated);
    assert_eq!(kept.title, "Time off");
    assert_eq!(
        kept.app, shell,
        "kept on the shell, never on the reviewed app (APP-085)"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}'")).await,
        reviewed_publications,
        "after generation, adaptation and keeping, the app under review still has only its own publications"
    );
    let after = footprint(&fixture).await;
    let mut expected = before.clone();
    // One revision, input, schema, publication, its input and schema, work,
    // collection and binding.
    for count in &mut expected[..=8] {
        *count += 1;
    }
    expected[10] += 4; // definition.drafted, publication.published, work.saved, work.bound
    assert_eq!(
        after, expected,
        "no source dependency, app, permission or grant row"
    );
    let outline = stores
        .data
        .work_outline(&member, &kept.app, &kept.work)
        .await
        .unwrap();
    assert!(
        outline.views.is_empty(),
        "no data view: its records card shows the records"
    );
    // APP-135: reconstruct the canvas provenance from a fresh database
    // read, not the transient composition response. This also guards the
    // admitted-description marker surviving draft/publish/save/reload.
    let (label, origins) = crate::query::canvas::definition_provenance(
        &outline.description,
        &outline.definition_digest,
    );
    assert_eq!(origins[0].kind, "generated");
    assert!(label.starts_with("Generated"));
    assert!(origins.iter().all(|origin| origin.kind != "seeded"));
    check(
        &mut checks,
        "APP-132/APP-135: a generated records-only definition (records input, filing create with initial, decision command, form and note; no source input and no data view) is admitted, drafted, owner-published and saved with one records binding and its record collection, still labeled generated from the persisted definition on reload, with no source dependency, app, permission or grant row",
    );

    // --- APP-191 (D7, D8): private entries and a reviewer-only command from plain words ---
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(prompt::private_entries_example().to_string()),
    ]);
    let private = compose_for(
        &stores,
        &composer,
        &member,
        "Collect suggestions from everyone; each person sees only the suggestions they sent in, and only reviewers can accept one",
    )
    .await
    .unwrap();
    assert_eq!(script.calls(), 2);
    assert_eq!(private.origin, Origin::Generated);
    assert_eq!(private.title, "Suggestion box");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{}' AND schema_id='suggestion' AND owner_only", private.app)).await,
        1,
        "the stored canonical schema derives owner_only"
    );
    let listing = stores
        .data
        .list_records(&member, &private.app, &private.work)
        .await
        .unwrap();
    let suggestions = listing.collections[0].collection.clone();
    let commands = |listing: &RecordsListing| -> Vec<String> {
        listing.collections[0]
            .commands
            .iter()
            .map(|c| c.id.clone())
            .collect()
    };
    assert_eq!(
        commands(&listing),
        ["accept", "suggest"],
        "the owner, a reviewer, sees accept"
    );
    let suggest = CommandRequest {
        app: private.app.clone(),
        work: private.work.clone(),
        publication: listing.publication.clone(),
        command: "suggest".into(),
        schema: "suggestion".into(),
        schema_version_expected: 1,
        record: "00000000-0000-4000-8000-000000000a91".into(),
        expected_record_version: 0,
        expected_work_version: listing.work_version,
        values: json!([
            {"field_id": "title", "value": {"type": "text", "value": "a suggestion"}},
            {"field_id": "details", "value": {"type": "null"}},
            {"field_id": "status", "value": {"type": "enum", "value": "new"}}
        ]),
        idempotency_key: "a9".repeat(16),
    };
    assert!(matches!(
        stores.data.commit_record(&member, &suggest).await.unwrap(),
        CommandResult::Committed(_)
    ));
    // A contributor shared in (D9) sees no accept, cannot run it, and sees no
    // suggestion but their own.
    let shared_in = request(&stores, ORG, AUTHOR, 1);
    stores
        .workspace
        .share_work(
            &member,
            &private.app,
            &private.work,
            Audience::Member(AUTHOR),
            ShareRole::Contributor,
        )
        .await
        .unwrap();
    let theirs = stores
        .data
        .list_records(&shared_in, &private.app, &private.work)
        .await
        .unwrap();
    assert_eq!(commands(&theirs), ["suggest"]);
    assert!(
        theirs.records.is_empty(),
        "the owner's suggestion is not theirs to see"
    );
    let accept = CommandRequest {
        command: "accept".into(),
        values: json!([{"field_id": "status", "value": {"type": "enum", "value": "accepted"}}]),
        expected_record_version: 1,
        idempotency_key: "a8".repeat(16),
        ..suggest.clone()
    };
    assert_eq!(
        stores.data.dry_run_command(&shared_in, &accept).await.err(),
        Some(Error::Denied)
    );
    let _ = suggestions;
    // APP-193 finding: an owner-advanced shared item is a reviewer-only update
    // the owner can run on the row they created (a declared transition would
    // refuse its filer).
    let item_fields = json!([
        {"id": "title", "value_type": {"type": "text"}, "nullable": false},
        {"id": "state", "value_type": {"type": "enum", "values": ["open", "closed"]}, "nullable": false}
    ]);
    let current_item = json!({
        "title": "Current item",
        "description": "One shared item at a time: its owner opens the next one and closes it.",
        "inputs": [{"id": "items", "contract_id": "item_v1", "contract_version": 1, "kind": "records", "fields": item_fields}],
        "views": [
            {"id": "open_item", "kind": "form", "renderer_version": 1, "title": "Open an item", "command_id": "open_item"},
            {"id": "how", "kind": "note", "renderer_version": 1, "title": "How it works", "text": "Everyone sees the current item; its owner advances it."}
        ],
        "record_schemas": [{"id": "item", "version": 1, "fields": item_fields}],
        "commands": [
            {"id": "open_item", "record_schema_id": "item", "operation": "create", "writable_fields": ["title", "state"], "transitions": [], "initial": [{"field_id": "state", "value": "open"}]},
            {"id": "advance", "record_schema_id": "item", "operation": "update", "writable_fields": ["state"], "transitions": [], "reviewer_only": true}
        ],
        "links": [],
        "default_layout": [{"view_id": "open_item", "column": 0, "row": 0, "width": 12}, {"view_id": "how", "column": 0, "row": 1, "width": 12}]
    });
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(current_item.to_string()),
    ]);
    let item = compose_for(
        &stores,
        &composer,
        &member,
        "One shared current item that I advance and close",
    )
    .await
    .unwrap();
    assert_eq!(script.calls(), 2);
    let listing = stores
        .data
        .list_records(&member, &item.app, &item.work)
        .await
        .unwrap();
    let open = CommandRequest {
        app: item.app.clone(),
        work: item.work.clone(),
        publication: listing.publication.clone(),
        command: "open_item".into(),
        schema: "item".into(),
        schema_version_expected: 1,
        record: "00000000-0000-4000-8000-000000000a92".into(),
        expected_record_version: 0,
        expected_work_version: listing.work_version,
        values: json!([
            {"field_id": "title", "value": {"type": "text", "value": "first"}},
            {"field_id": "state", "value": {"type": "enum", "value": "open"}}
        ]),
        idempotency_key: "a7".repeat(16),
    };
    assert!(matches!(
        stores.data.commit_record(&member, &open).await.unwrap(),
        CommandResult::Committed(_)
    ));
    let advance = CommandRequest {
        command: "advance".into(),
        values: json!([{"field_id": "state", "value": {"type": "enum", "value": "closed"}}]),
        expected_record_version: 1,
        idempotency_key: "a6".repeat(16),
        ..open.clone()
    };
    let CommandResult::Committed(advanced) =
        stores.data.commit_record(&member, &advance).await.unwrap()
    else {
        panic!("the owner advances their own item");
    };
    assert_eq!(advanced.record_version, 2);
    check(
        &mut checks,
        "APP-191 (D7, D8), APP-193: from plain words the composer generates a collection private to each member (owner_only derived in the database) and a reviewer-only accept the owner sees and a shared-in contributor neither sees nor runs; an owner-advanced shared item is a reviewer-only update its creator can commit",
    );
    let after = footprint(&fixture).await;

    // --- A model failure writes nothing ---
    let before = after;
    let (_, composer) = Script::composer(vec![
        Err(ModelError::Unavailable),
        Err(ModelError::Unavailable),
    ]);
    assert_eq!(
        compose_for(&stores, &composer, &member, intent).await,
        Err(Refused::CouldNotCompose("model_unavailable"))
    );
    assert_eq!(footprint(&fixture).await, before);
    // A revoked member stays the uniform refusal. The scripted router reply
    // is never consulted: membership fails before a model call, and no row
    // is written. The ungranted teammate's read denial is the earlier check.
    let (script, composer) = Script::composer(vec![Ok(router_reply(APP, "high", ""))]);
    assert_eq!(
        compose_for(&stores, &composer, &revoked, intent).await,
        Err(Refused::NotFound)
    );
    assert_eq!(script.calls(), 0);
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "a model failure after the router is the neutral could_not_compose with no row written; a revoked member stays refused and no row is written",
    );

    // --- APP-145: one shell when an active member owns no eligible app ---
    // The member above owns APP, which is under review and so not eligible
    // (APP-085), and the shell its first compose created. A fresh context
    // selects that shell and does not insert another. The bare member owns
    // nothing.
    assert_eq!(
        stores
            .workspace
            .composer_context(&member)
            .await
            .unwrap()
            .authoring
            .as_ref()
            .map(|authoring| authoring.app.as_str()),
        Some(shell.as_str())
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{PRINCIPAL}'")).await,
        2,
        "APP under review and one shell: a member who already has an eligible app composes without another shell"
    );
    let policies = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.publication_policies WHERE organization_id='{ORG}'"),
    )
    .await;
    let granted_before = chain_events(&fixture, "admission", "permission.granted").await;
    let bare = request(&stores, ORG, BARE, 1);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{BARE}'")).await,
        0
    );
    let before_listing = footprint(&fixture).await;
    let listed = stores.workspace.composer_context(&bare).await.unwrap();
    assert!(listed.authoring.is_none());
    assert_eq!(
        footprint(&fixture).await,
        before_listing,
        "read-only command discovery must not create an app, grant or audit row"
    );
    // Hold the shell lock until both preparations are observed waiting on it.
    // They must not try to upgrade the member fence to FOR UPDATE, and the
    // second waiter must reuse the first transaction's shell after it commits.
    let holder = fixture.operator().await;
    let lock = format!("genui:authoring-shell:{ORG}:{BARE}");
    holder
        .client
        .query_one(
            "SELECT pg_advisory_lock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    let pending = async {
        tokio::join!(
            stores.workspace.prepare_composer(&bare),
            stores.workspace.prepare_composer(&bare)
        )
    };
    tokio::pin!(pending);
    let mut waiting = 0_i64;
    for _ in 0..40 {
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending)
                .await
                .is_err(),
            "both first composes wait for the shell lock"
        );
        waiting = holder.client.query_one(
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname='genui')", &[])
            .await.unwrap().get(0);
        if waiting == 2 {
            break;
        }
    }
    assert_eq!(waiting, 2, "two first composes reached the shell lock");
    holder
        .client
        .query_one(
            "SELECT pg_advisory_unlock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    let (first, concurrent) = pending.await;
    let first = first.unwrap();
    assert_eq!(
        first.authoring,
        concurrent.unwrap().authoring,
        "overlapping first composes return the same shell without deadlock"
    );
    drop(holder);
    let shell = first
        .authoring
        .as_ref()
        .expect("one authoring shell")
        .app
        .clone();
    assert_ne!(shell, APP);
    assert!(first.library.is_empty() && first.sources.is_empty());
    assert_eq!(
        first.owner_publish_policy.as_deref(),
        Some(POLICY),
        "the policy row is the organization's, not a grant"
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!(
                "SELECT count(*) FROM genui.publication_policies WHERE organization_id='{ORG}'"
            )
        )
        .await,
        policies,
        "the shell does not invent a publication policy"
    );
    assert_eq!(
        text(&fixture, &format!("SELECT owner_principal_id::text FROM genui.apps WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        BARE
    );
    assert_eq!(
        text(&fixture, &format!("SELECT space_id::text FROM genui.apps WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        text(&fixture, &format!("SELECT space_id::text FROM genui.spaces WHERE organization_id='{ORG}' ORDER BY space_id LIMIT 1")).await,
        "the shell uses a space the organization already has"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        0,
        "the shell is not a library app and has no publication"
    );
    assert_eq!(
        fixture
            .sql(
                &format!("SELECT permission FROM genui.app_permissions WHERE organization_id='{ORG}' AND principal_id='{BARE}' AND app_id='{shell}' ORDER BY 1"),
                "genui",
            )
            .trim(),
        "discover\nedit_definition\nrun"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.app_permissions WHERE organization_id='{ORG}' AND principal_id='{BARE}'")).await,
        3
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{BARE}'")).await,
        0
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.source_permissions WHERE organization_id='{ORG}' AND principal_id='{BARE}'")).await,
        0
    );
    assert_eq!(
        chain_events(&fixture, "admission", "permission.granted").await,
        granted_before + 1
    );
    assert_eq!(
        text(&fixture, &format!("SELECT app_id::text||':'||actor_principal_id::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='permission.granted' AND actor_principal_id='{BARE}'")).await,
        format!("{shell}:{BARE}")
    );
    let second = stores.workspace.composer_context(&bare).await.unwrap();
    assert_eq!(
        second
            .authoring
            .as_ref()
            .map(|authoring| authoring.app.as_str()),
        Some(shell.as_str())
    );
    assert!(second.library.iter().all(|app| app.app != shell));
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{BARE}'")).await,
        1,
        "a second compose does not create a second shell"
    );
    assert_eq!(
        chain_events(&fixture, "admission", "permission.granted").await,
        granted_before + 1,
        "a second compose does not grant again"
    );

    let time_off: Value = serde_json::from_str(include_str!(
        "../../../contracts/fixtures/library/time-off.json"
    ))
    .unwrap();
    let records_only = prompt::records_example(time_off["definition"]["content"].clone());
    let (script, composer) = Script::composer(vec![
        Ok(with(
            records_only.clone(),
            "/views/0/script",
            json!("<script>alert(1)</script>"),
        )
        .to_string()),
        Ok(with(
            records_only.clone(),
            "/permissions",
            json!(["admin_policy"]),
        )
        .to_string()),
        Ok(with(
            records_only.clone(),
            "/views/0/sql",
            json!("DROP TABLE genui.apps; --"),
        )
        .to_string()),
    ]);
    assert_eq!(
        compose_for(
            &stores,
            &composer,
            &bare,
            "Track the team's time-off requests",
        )
        .await,
        Err(Refused::CouldNotCompose("admission_refused"))
    );
    assert_eq!(
        script.calls(),
        3,
        "an empty library generates; three refusals and no router call"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        0
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_revisions WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        0,
        "a hostile reply does not publish and does not draft"
    );

    let (script, composer) = Script::composer(vec![Ok(records_only.to_string())]);
    let composed = compose_for(
        &stores,
        &composer,
        &bare,
        "Track the team's time-off requests",
    )
    .await
    .unwrap();
    assert_eq!(script.calls(), 1, "one generation call and no router call");
    assert_eq!(composed.origin, Origin::Generated);
    assert_eq!(composed.app, shell);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{BARE}'")).await,
        1
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{shell}'")).await,
        1
    );
    assert_eq!(
        text(&fixture, &format!("SELECT app_id::text FROM genui.saved_work WHERE organization_id='{ORG}' AND work_id='{}'", composed.work)).await,
        shell
    );
    assert_eq!(
        chain_events(&fixture, "admission", "permission.granted").await,
        granted_before + 1
    );
    let third = stores.workspace.composer_context(&bare).await.unwrap();
    assert_eq!(
        third
            .authoring
            .as_ref()
            .map(|authoring| authoring.app.as_str()),
        Some(shell.as_str()),
        "choose_authoring keeps the shell after it is published"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{BARE}'")).await,
        1
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{PRINCIPAL}'")).await,
        2,
        "the member owns APP (under review) and its one authoring shell (APP-085)"
    );
    check(
        &mut checks,
        "APP-145: read-only command discovery creates nothing; two overlapping first composes wait on the member shell lock and receive the same empty shell (edit_definition, run, discover, one permission.granted naming that app); a second compose reuses it; a hostile reply is refused and does not draft; a records-only definition is admitted and published on that shell and not as a second app; a member who already owns an eligible app gains no shell and no publication policy is inserted",
    );

    // A first compose can finish publishing while another prepare is queued.
    // Force that ordering: the holder creates the shell on its own backend
    // (the advisory lock is reentrant), and publication uses the normal
    // admitted draft/publish/save path. The queued context must select that
    // now-published shell, rather than create another from a stale empty read.
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{AUTHOR}'")).await, 0);
    let holder = fixture.operator().await;
    let lock = format!("genui:authoring-shell:{ORG}:{AUTHOR}");
    holder
        .client
        .query_one(
            "SELECT pg_advisory_lock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    let pending = stores.workspace.prepare_composer(&stranger);
    tokio::pin!(pending);
    let mut waiting = 0_i64;
    for _ in 0..40 {
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending)
                .await
                .is_err(),
            "the next first compose waits for the shell lock"
        );
        waiting = holder.client.query_one(
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname='genui')", &[])
            .await.unwrap().get(0);
        if waiting == 1 {
            break;
        }
    }
    assert_eq!(waiting, 1);
    holder.client.batch_execute(&format!(
        "BEGIN; SET LOCAL ROLE genui_workspace; SELECT set_config('genui.organization_id','{ORG}',true); SELECT set_config('genui.principal_id','{AUTHOR}',true); SELECT set_config('genui.membership_epoch','1',true); SELECT genui_private.ensure_authoring_shell('{ORG}','{AUTHOR}'); COMMIT;"
    )).await.unwrap();
    let context = stores.workspace.composer_context(&stranger).await.unwrap();
    let published_shell = context.authoring.as_ref().unwrap().app.clone();
    let planned = plan::plan_generated(
        &records_only.to_string(),
        &context.sources,
        ORG,
        &published_shell,
    )
    .unwrap();
    crate::composer::routes::publish_planned(
        &stores,
        &stranger,
        &context,
        planned,
        "Time off requests",
    )
    .await
    .unwrap();
    holder
        .client
        .query_one(
            "SELECT pg_advisory_unlock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    let resumed = pending.await.unwrap();
    assert_eq!(
        resumed.authoring.as_ref().map(|app| app.app.as_str()),
        Some(published_shell.as_str()),
        "a queued first compose must reuse a shell published before its lock resumes"
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{AUTHOR}'")).await, 1);
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND actor_principal_id='{AUTHOR}' AND kind='permission.granted'")).await, 1);
    drop(holder);
    check(
        &mut checks,
        "APP-145: a queued first compose re-reads the owned apps after the shell lock, reusing a shell published while it waited; one app and one grant audit remain",
    );

    // No owner_publish policy: the shell may be created and must not publish.
    fixture.sql(
        &format!(
            "BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{EMPTY_ORG}',true); \
             INSERT INTO genui.organizations (organization_id,display_name,status) VALUES ('{EMPTY_ORG}','Empty organization','active'); \
             INSERT INTO genui.provider_tenant_bindings (organization_id,issuer,provider_tenant_id,active) VALUES ('{EMPTY_ORG}','app008-fixture-only','ten_{EMPTY_ORG}',true); \
             INSERT INTO genui.principals (organization_id,principal_id,issuer,provider_tenant_id,provider_subject) VALUES ('{EMPTY_ORG}','{EMPTY_MEMBER}','app008-fixture-only','ten_{EMPTY_ORG}','usr_{EMPTY_MEMBER}'); \
             INSERT INTO genui.memberships (organization_id,principal_id,status,membership_epoch) VALUES ('{EMPTY_ORG}','{EMPTY_MEMBER}','active',1); \
             INSERT INTO genui.entitlements (organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at,revoked_at) VALUES ('{EMPTY_ORG}','{EMPTY_MEMBER}','{EMPTY_ENTITLEMENT}','active','member_invite','2026-10-01T00:00:00+00:00','2036-10-01T00:00:00+00:00',NULL); \
             INSERT INTO genui.spaces (organization_id,space_id,display_name) VALUES ('{EMPTY_ORG}','{EMPTY_SPACE}','Empty space'); \
             COMMIT;"
        ),
        "genui",
    );
    let created = operator(&fixture, &["backfill", "--organization", EMPTY_ORG]);
    assert_eq!(created["created"].as_array().unwrap().len(), 5);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publication_policies WHERE organization_id='{EMPTY_ORG}'")).await,
        0
    );
    let empty_member = request(&stores, EMPTY_ORG, EMPTY_MEMBER, 1);
    // Expiry can pass while the authoring lock is queued, after the member
    // fence's entitlement read. The first write must recheck the wall clock.
    let holder = fixture.operator().await;
    let lock = format!("genui:authoring-shell:{EMPTY_ORG}:{EMPTY_MEMBER}");
    holder
        .client
        .query_one(
            "SELECT pg_advisory_lock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    let pending = stores.workspace.prepare_composer(&empty_member);
    tokio::pin!(pending);
    let mut waiting = 0_i64;
    for _ in 0..40 {
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending)
                .await
                .is_err(),
            "the new member is queued before entitlement expiry"
        );
        waiting = holder.client.query_one(
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname='genui')", &[])
            .await.unwrap().get(0);
        if waiting == 1 {
            break;
        }
    }
    assert_eq!(waiting, 1);
    holder.client.batch_execute(&format!("UPDATE genui.entitlements SET expires_at=starts_at+interval '1 millisecond' WHERE organization_id='{EMPTY_ORG}' AND principal_id='{EMPTY_MEMBER}'")).await.unwrap();
    holder
        .client
        .query_one(
            "SELECT pg_advisory_unlock(hashtextextended($1::text,0))::text",
            &[&lock],
        )
        .await
        .unwrap();
    assert_eq!(
        pending.await,
        Err(Error::Denied),
        "expiry while queued refuses before creating a shell"
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{EMPTY_ORG}'")
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!(
                "SELECT count(*) FROM genui.app_permissions WHERE organization_id='{EMPTY_ORG}'"
            )
        )
        .await,
        0
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{EMPTY_ORG}' AND kind='permission.granted'")).await, 0);
    holder.client.batch_execute(&format!("UPDATE genui.entitlements SET expires_at='2036-10-01T00:00:00Z' WHERE organization_id='{EMPTY_ORG}' AND principal_id='{EMPTY_MEMBER}'")).await.unwrap();
    drop(holder);
    check(
        &mut checks,
        "APP-145: entitlement expiry while waiting for the authoring lock is rechecked before any app, permission or audit write",
    );
    let empty_context = stores
        .workspace
        .prepare_composer(&empty_member)
        .await
        .unwrap();
    let empty_shell = empty_context
        .authoring
        .as_ref()
        .expect("shell without a publication policy")
        .app
        .clone();
    assert!(empty_context.owner_publish_policy.is_none());
    let (script, composer) = Script::composer(vec![Ok(records_only.to_string())]);
    assert_eq!(
        compose_for(
            &stores,
            &composer,
            &empty_member,
            "Track the team's time-off requests",
        )
        .await,
        Err(Refused::CouldNotCompose("no_owner_publish_policy"))
    );
    assert_eq!(script.calls(), 1);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{EMPTY_ORG}' AND owner_principal_id='{EMPTY_MEMBER}'")).await,
        1
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{EMPTY_ORG}' AND app_id='{empty_shell}'")).await,
        0
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_revisions WHERE organization_id='{EMPTY_ORG}' AND app_id='{empty_shell}'")).await,
        0,
        "without owner_publish the shell does not publish"
    );
    check(
        &mut checks,
        "APP-145: an organization with no owner_publish policy still refuses no_owner_publish_policy; the shell is created and nothing is drafted or published",
    );

    // APP-141: catalog definitions create real empty works without a model,
    // including the first intentional compose of a member who owns no app.
    let template_member_id = "00000000-0000-4000-8000-000000000148";
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); \
        INSERT INTO genui.principals(organization_id,principal_id,issuer,provider_tenant_id,provider_subject) VALUES('{ORG}','{template_member_id}','app008-fixture-only','ten_{ORG}','usr_{template_member_id}'); \
        INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES('{ORG}','{template_member_id}','active',1); \
        INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES('{ORG}','{template_member_id}','0000e000-0000-4000-8000-000000000148','active','member_invite','2026-10-01T00:00:00Z','2036-10-01T00:00:00Z'); COMMIT;"), "genui");
    let template_member = request(&stores, ORG, template_member_id, 1);
    assert!(stores
        .workspace
        .composer_context(&template_member)
        .await
        .unwrap()
        .authoring
        .is_none());
    let no_model = Composer::new(None, "router-model", "generation-model");
    let template = crate::composer::templates::find("equipment-requests").unwrap();
    // Keep the pin outside the futures: compose awaits through the whole
    // publication and saved-work transaction sequence.
    let pin = crate::composer::routes::Pin::Template(template.id.to_owned());
    let crate::composer::routes::First::Placed(first_template, _) =
        crate::composer::routes::compose_instant(
            &stores,
            &no_model,
            &template_member,
            template.description,
            &pin,
        )
        .await
        .unwrap()
    else {
        panic!("template cannot choose a different app");
    };
    let crate::composer::routes::First::Placed(second_template, _) =
        crate::composer::routes::compose_instant(
            &stores,
            &no_model,
            &template_member,
            template.description,
            &pin,
        )
        .await
        .unwrap()
    else {
        panic!("template cannot choose a different app");
    };
    assert_eq!(first_template.app, second_template.app);
    assert_ne!(first_template.work, second_template.work);
    assert_eq!(first_template.origin, Origin::Adapted);
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}' AND owner_principal_id='{template_member_id}'")).await, 1);
    let first_outline = stores
        .data
        .work_outline(&template_member, &first_template.app, &first_template.work)
        .await
        .unwrap();
    assert_eq!(first_outline.title, template.title);
    assert_eq!(first_outline.parts[0].id, "guide");
    assert!(stores
        .data
        .list_records(&template_member, &first_template.app, &first_template.work)
        .await
        .unwrap()
        .records
        .is_empty());
    check(
        &mut checks,
        "APP-141: an unseeded member discovers without creating a shell, then confirms two exact admitted templates without a model; one authoring shell, two distinct empty works, declared guide-first layout and honest adapted provenance",
    );

    let label_before = stores
        .workspace
        .work_label(
            &template_member,
            &second_template.app,
            &second_template.work,
        )
        .await
        .unwrap();
    assert_eq!(label_before.version, 0);
    assert!(label_before.can_name);
    let saved = stores
        .workspace
        .label_work(
            &template_member,
            &second_template.app,
            &second_template.work,
            Some("Design team equipment"),
            0,
        )
        .await
        .unwrap();
    assert!(matches!(saved, WorkLabelResult::Saved(_)));
    assert_eq!(
        stores
            .data
            .work_outline(&template_member, &first_template.app, &first_template.work)
            .await
            .unwrap(),
        first_outline
    );
    assert_eq!(
        stores
            .data
            .work_outline(
                &template_member,
                &second_template.app,
                &second_template.work
            )
            .await
            .unwrap()
            .title,
        "Design team equipment"
    );
    // Name discovery must use the same exact work label as the document.
    // The operator fixture marks both as saved commands; it grants nothing.
    fixture.sql(&format!(
        "UPDATE genui.saved_work SET context='forma-command:/first-template' WHERE organization_id='{ORG}' AND app_id='{}' AND work_id='{}'; \
         UPDATE genui.saved_work SET context='forma-command:/second-template' WHERE organization_id='{ORG}' AND app_id='{}' AND work_id='{}';",
        first_template.app, first_template.work, second_template.app, second_template.work
    ), "genui");
    let listed_names = stores.data.openable_works(&template_member).await.unwrap();
    assert_eq!(
        listed_names
            .iter()
            .find(|w| w.work == first_template.work)
            .unwrap()
            .title,
        template.title
    );
    assert_eq!(
        listed_names
            .iter()
            .find(|w| w.work == second_template.work)
            .unwrap()
            .title,
        "Design team equipment"
    );
    assert_eq!(
        listed_names,
        stores
            .data
            .openable_works_serial(&template_member)
            .await
            .unwrap()
    );
    let command_names = stores.data.named_works(&template_member).await.unwrap();
    assert_eq!(
        command_names
            .iter()
            .find(|w| w.work.work == first_template.work)
            .unwrap()
            .work
            .title,
        template.title
    );
    assert_eq!(
        command_names
            .iter()
            .find(|w| w.work.work == second_template.work)
            .unwrap()
            .work
            .title,
        "Design team equipment"
    );
    assert_eq!(
        stores
            .workspace
            .label_work(
                &template_member,
                &second_template.app,
                &second_template.work,
                Some("Stale name"),
                0
            )
            .await
            .unwrap(),
        WorkLabelResult::Conflict
    );
    assert_eq!(
        stores
            .workspace
            .label_work(
                &foreign,
                &second_template.app,
                &second_template.work,
                Some("Foreign name"),
                1
            )
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .label_work(
                &member,
                &second_template.app,
                &second_template.work,
                Some("Other member"),
                1
            )
            .await,
        Err(Error::Denied)
    );
    let (left, right) = tokio::join!(
        stores.workspace.label_work(
            &template_member,
            &second_template.app,
            &second_template.work,
            Some("Team A equipment"),
            1
        ),
        stores.workspace.label_work(
            &template_member,
            &second_template.app,
            &second_template.work,
            Some("Team B equipment"),
            1
        ),
    );
    assert!(matches!(
        (&left, &right),
        (Ok(WorkLabelResult::Saved(_)), Ok(WorkLabelResult::Conflict))
            | (Ok(WorkLabelResult::Conflict), Ok(WorkLabelResult::Saved(_)))
    ));
    assert_eq!(scalar(&fixture, &format!("SELECT version FROM genui.saved_work WHERE organization_id='{ORG}' AND app_id='{}' AND work_id='{}'", second_template.app, second_template.work)).await, 1, "display naming cannot advance publication/work authority pins");
    check(
        &mut checks,
        "APP-141: renaming the second work leaves its sibling's title, definition and layout unchanged; names survive authorized reads; stale and concurrent versions conflict, foreign/ungranted callers deny, and the saved-work authority version stays unchanged",
    );

    super::work_presentation_checks::run(
        &fixture,
        &stores,
        &template_member,
        &first_template,
        &second_template,
        &mut checks,
    )
    .await;

    // A second rename may wait on the exact work after its entitlement
    // precheck. Expiry while queued must leave the name and audit unchanged.
    let labels_before = chain_events(&fixture, "workspace", "app.labeled").await;
    let holder = fixture.operator().await;
    holder.client.batch_execute("BEGIN").await.unwrap();
    let before = holder.client.query_one(
        "SELECT display_name,name_version FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid FOR UPDATE",
        &[&ORG, &second_template.app, &second_template.work],
    ).await.unwrap();
    let held_name: Option<String> = before.get(0);
    let held_version: i64 = before.get(1);
    let pending = stores.workspace.label_work(
        &template_member,
        &second_template.app,
        &second_template.work,
        Some("Expired rename"),
        held_version,
    );
    tokio::pin!(pending);
    let mut waiting = 0_i64;
    for _ in 0..40 {
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending)
                .await
                .is_err(),
            "rename reaches the held work row before expiry"
        );
        waiting = holder.client.query_one(
            "SELECT count(*) FROM pg_locks waiting JOIN pg_locks held USING(locktype,transactionid) WHERE waiting.locktype='transactionid' AND NOT waiting.granted AND held.pid=pg_backend_pid() AND held.granted", &[])
            .await.unwrap().get(0);
        if waiting == 1 {
            break;
        }
    }
    assert_eq!(waiting, 1, "rename is blocked on this work row");
    holder.client.batch_execute(&format!("UPDATE genui.entitlements SET expires_at=starts_at+interval '1 millisecond' WHERE organization_id='{ORG}' AND principal_id='{template_member_id}'; COMMIT")).await.unwrap();
    assert_eq!(pending.await, Err(Error::Denied));
    let after = holder.client.query_one(
        "SELECT display_name,name_version FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid",
        &[&ORG, &second_template.app, &second_template.work],
    ).await.unwrap();
    assert_eq!(after.get::<_, Option<String>>(0), held_name);
    assert_eq!(after.get::<_, i64>(1), held_version);
    assert_eq!(
        chain_events(&fixture, "workspace", "app.labeled").await,
        labels_before
    );
    holder.client.batch_execute(&format!("UPDATE genui.entitlements SET expires_at='2036-10-01T00:00:00Z' WHERE organization_id='{ORG}' AND principal_id='{template_member_id}'")).await.unwrap();
    drop(holder);
    check(
        &mut checks,
        "APP-141: entitlement expiry while a rename waits for its work row denies the mutation, preserving both name/version and the audit count",
    );

    let _: Option<Composed> = None;
    let _ = plan::Decision::Generate;
    let report = json!({"campaign": "APP-093 composer", "passed": true, "checks": checks});
    if let Some(path) = std::env::var_os("GENUI_COMPOSER_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    stores.close().await.unwrap();
}
