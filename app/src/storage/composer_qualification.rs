//! APP-093 composer campaign. Isolated test executable only: local Docker,
//! OpenSSL, a throwaway CA and the locked Python environments, exactly as the
//! APP-010 definition campaign. A scripted model (nothing here calls the real
//! endpoint) drives `composer::routes::compose_for` over real rows: the
//! member's context names only what the member owns, may read and may run;
//! a stranger to every app composes nothing (the uniform refusal); a foreign
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
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let member = request(&stores, ORG, PRINCIPAL, 1);
    let stranger = request(&stores, ORG, AUTHOR, 1);
    let foreign = request(&stores, OTHER, PRINCIPAL, 1);
    let intent = "Which destinations receive the most bytes each minute?";

    // --- The context names only what the member owns, reads and runs ---
    let context = stores.workspace.composer_context(&member).await.unwrap();
    assert_eq!(
        context.authoring.as_ref().map(|a| a.app.as_str()),
        Some(APP)
    );
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
    let stranger_context = stores.workspace.composer_context(&stranger).await.unwrap();
    assert_eq!(stranger_context.authoring, None);
    assert!(stranger_context.sources.is_empty() && stranger_context.library.is_empty());
    assert_eq!(
        stranger_context.owner_publish_policy.as_deref(),
        Some(POLICY),
        "the policy row is the organization's, not a grant"
    );
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
        "the composer context names the owned authoring app, the owner_publish policy, the two readable source revisions and the one bindable library app; a member without grants sees nothing; a foreign organization's apps are never offered",
    );

    // --- Nothing composable, disabled, and a model that is never consulted ---
    let (script, composer) = Script::composer(vec![Ok(router_reply(APP, "high", "x"))]);
    assert_eq!(
        compose_for(&stores, &composer, &stranger, intent).await,
        Err(Refused::NotFound)
    );
    assert_eq!(
        script.calls(),
        0,
        "no model call for a member who may compose nothing"
    );
    let disabled = Composer::new(None, "r", "g");
    assert_eq!(
        compose_for(&stores, &disabled, &member, intent).await,
        Err(Refused::ComposerUnavailable)
    );
    let before = footprint(&fixture).await;
    check(
        &mut checks,
        "a member who may compose nothing gets the uniform refusal before any model call; without a key the composer answers composer_unavailable; neither writes a row",
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

    // --- Generated: admitted, drafted on the owned app, owner-published, saved ---
    let before = after;
    let publications_before = scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}'")).await;
    let (script, composer) = Script::composer(vec![
        Ok(router_reply("none", "low", "")),
        Ok(serde_json::to_string(&runnable_northstar()).unwrap()),
    ]);
    let generated = compose_for(&stores, &composer, &member, intent)
        .await
        .unwrap();
    assert_eq!(script.calls(), 2);
    assert_eq!(generated.origin, Origin::Generated);
    assert_eq!(generated.app, APP);
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
    let publications_after = scalar(&fixture, &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}'")).await;
    assert_eq!(publications_after, publications_before + 1);
    let detail = stores
        .workspace
        .read_work(&member, APP, &generated.work)
        .await
        .unwrap();
    assert_ne!(
        detail.publication, context.library[0].publication,
        "pinned to the new publication"
    );
    let newest = text(&fixture, &format!("SELECT publication_id::text FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}' ORDER BY publication_sequence DESC LIMIT 1")).await;
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
        .work_outline(&member, APP, &generated.work)
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
        "a generated definition passes admission, is drafted on the member's owned app, owner-published (author and approver the member) and saved bound to the registered source revisions, labeled generated, with definition.drafted, publication.published, work.saved and work.bound events and no new app, permission or grant row",
    );

    // --- APP-132: a records-only app (no source input) is generated and saved ---
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
        .work_outline(&member, APP, &kept.work)
        .await
        .unwrap();
    assert!(
        outline.views.is_empty(),
        "no data view: its records card shows the records"
    );
    check(
        &mut checks,
        "APP-132: a generated records-only definition (records input, filing create with initial, decision command, form and note; no source input and no data view) is admitted, drafted, owner-published and saved with one records binding and its record collection, labeled generated, with no source dependency, app, permission or grant row",
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
    let after = footprint(&fixture).await;
    assert_eq!(after[3], before[3] + 1, "one more publication");
    assert_eq!(after[6], before[6] + 1, "one more saved work");
    assert_eq!(after[11..], before[11..], "no app, permission or grant row");
    let outline = stores
        .data
        .work_outline(&member, APP, &adapted.work)
        .await
        .unwrap();
    assert_eq!(outline.title, "Traffic review");
    assert_eq!(
        outline.views.len(),
        2,
        "the seeded Northstar runnable views (total, timeline), patched title"
    );
    check(
        &mut checks,
        "a medium-confidence router choice becomes an adapted definition (title and description patched, re-admitted whole), owner-published on the member's app and saved, labeled adapted",
    );

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
    // A member who may not draft on any app (the stranger holds nothing) is
    // the uniform refusal even when the library would have answered.
    let (_, composer) = Script::composer(vec![Ok(router_reply(APP, "high", ""))]);
    assert_eq!(
        compose_for(&stores, &composer, &stranger, intent).await,
        Err(Refused::NotFound)
    );
    check(
        &mut checks,
        "a model failure after the router is the neutral could_not_compose with no row written; the stranger stays refused",
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
