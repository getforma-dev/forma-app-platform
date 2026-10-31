//! APP-108 route-level unit tests (no database): every route is mounted,
//! mutations refuse cross-site requests first, and without a session every
//! answer is the uniform 404.
use super::routes::{long_path, router, Boxes};
use crate::auth::Auth;
use axum::{body::Body, http::StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

const ID: &str = "00000000-0000-4000-8000-0000000000aa";

fn state() -> Arc<Boxes> {
    let auth = Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
    let work = Arc::new(crate::query::Work::new(
        auth.clone(),
        None,
        Arc::new(crate::query::routes::tests::RecordingPages::default()),
    ));
    Arc::new(Boxes {
        auth,
        work,
        composer: crate::composer::Composer::new(None, "r", "g"),
        store: super::store::ObjectStore::Local {
            root: std::env::temp_dir().join("genui-box-routes-unused"),
        },
        normalizer: super::register::Normalizer::image(),
        parser: None,
    })
}

fn app() -> axum::Router {
    crate::http::uniform(router(state()))
}

/// Review: every folder surface (the folder routes, `/v1/act`, Slack, MCP)
/// spends the same per-member windows as the canvas composer and the brief
/// route, so another surface cannot be used to go around them.
#[test]
fn folder_windows_are_the_composer_and_brief_windows() {
    use super::routes::{admit_compose, admit_explain, BuildRefusal, EXPLAIN_MEMBER};
    let state = state();
    let member =
        crate::storage::AgentCaller::for_tests(ID, "00000000-0000-4000-8000-0000000000bb", "t")
            .request;
    let compose = format!("compose:{ID}:{}", member.principal());
    for _ in 0..crate::composer::routes::COMPOSE_MEMBER[0].limit {
        assert!(admit_compose(&state, &member).is_ok());
    }
    assert!(state
        .auth
        .limits
        .would_admit(&[(&compose, crate::composer::routes::COMPOSE_MEMBER)])
        .is_err());
    assert!(matches!(
        admit_compose(&state, &member),
        Err(BuildRefusal::TooMany(_))
    ));
    let explain = format!("box-explain:{ID}:{}", member.principal());
    for _ in 0..EXPLAIN_MEMBER[0].limit {
        state
            .auth
            .limits
            .admit(&[(&explain, EXPLAIN_MEMBER)])
            .unwrap();
    }
    let refused = admit_explain(&state, &member).unwrap_err();
    assert_eq!(refused.code(), "too_many_requests");
    assert_eq!(refused.response().status(), StatusCode::TOO_MANY_REQUESTS);
}

async fn send(method: &str, uri: &str, site: &str) -> (StatusCode, String) {
    let request = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("sec-fetch-site", site)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = app().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn mutations_refuse_cross_site_first_and_everything_else_is_the_uniform_404() {
    let posts = [
        "/v1/boxes".to_owned(),
        "/v1/boxes/uploads".to_owned(),
        format!("/v1/boxes/{ID}/files"),
        format!("/v1/boxes/{ID}/files/{ID}/data"),
        format!("/v1/boxes/{ID}/files/{ID}/attach"),
        format!("/v1/boxes/{ID}/apps"),
        format!("/v1/boxes/{ID}/share"),
        format!("/v1/boxes/{ID}/name"),
        format!("/v1/boxes/{ID}/brief"),
        format!("/v1/boxes/{ID}/ask"),
        format!("/v1/boxes/explain/{ID}/{ID}"),
    ];
    for uri in &posts {
        assert_eq!(
            send("POST", uri, "cross-site").await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
        let (status, body) = send("POST", uri, "same-origin").await;
        assert_eq!(
            (status, body.as_str()),
            (
                StatusCode::NOT_FOUND,
                r#"{"code":"not_found","status":404}"#
            ),
            "{uri}"
        );
    }
    for uri in [
        "/v1/boxes".to_owned(),
        format!("/v1/boxes/{ID}"),
        format!("/v1/boxes/{ID}/files/{ID}/content"),
        format!("/v1/boxes/{ID}/files/{ID}/preview"),
        format!("/v1/boxes/{ID}/brief"),
        format!("/v1/boxes/lineage/{ID}/{ID}"),
        format!("/v1/boxes/explain/{ID}/{ID}"),
        "/v1/boxes/not-a-uuid".to_owned(),
    ] {
        let (status, body) = send("GET", &uri, "same-origin").await;
        assert_eq!(
            (status, body.as_str()),
            (
                StatusCode::NOT_FOUND,
                r#"{"code":"not_found","status":404}"#
            ),
            "{uri}"
        );
    }
}

#[test]
fn multipart_takes_exactly_one_file_part() {
    let ct = "multipart/form-data; boundary=XyZ";
    let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"q3.csv\"\r\nContent-Type: text/csv\r\n\r\na,b\n1,2\n\r\n--XyZ--\r\n";
    let (name, bytes) = super::routes::file_part(ct, body).unwrap();
    assert_eq!((name.as_str(), bytes), ("q3.csv", &b"a,b\n1,2\n"[..]));
    let two = b"--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\n\r\nx\r\n--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"b\"\r\n\r\ny\r\n--XyZ--\r\n";
    assert!(super::routes::file_part(ct, two).is_none());
    let other = b"--XyZ\r\nContent-Disposition: form-data; name=\"x\"\r\n\r\nx\r\n--XyZ--\r\n";
    assert!(super::routes::file_part(ct, other).is_none());
    assert!(super::routes::file_part("application/json", body).is_none());
}

#[test]
fn long_paths_are_exactly_the_parser_normalizer_and_model_routes() {
    for path in [
        format!("/v1/boxes/{ID}/files"),
        format!("/v1/boxes/{ID}/apps"),
        format!("/v1/boxes/{ID}/brief"),
        format!("/v1/boxes/{ID}/ask"),
        format!("/v1/boxes/{ID}/files/{ID}/preview"),
        format!("/v1/boxes/{ID}/files/{ID}/data"),
        format!("/v1/boxes/{ID}/files/{ID}/attach"),
        format!("/v1/boxes/explain/{ID}/{ID}"),
    ] {
        assert!(long_path(&path), "{path}");
    }
    for path in [
        "/v1/boxes".to_owned(),
        format!("/v1/boxes/{ID}"),
        format!("/v1/boxes/{ID}/files/{ID}/content"),
        format!("/v1/boxes/lineage/{ID}/{ID}"),
        format!("/v1/boxes/{ID}/share"),
        "/v1/compose".to_owned(),
        "/v1/act".to_owned(),
    ] {
        assert!(!long_path(&path), "{path}");
    }
}

/// The real-Haiku folder smoke (ADR-011 pitch; APP-108): the key file named
/// by `GENUI_BOX_SMOKE_KEY_FILE` is read into memory only (never printed,
/// logged or copied). Over the synthetic fixture folder it runs the derived
/// app's generation with the capped context (Haiku as the generation model),
/// the explanation over computed results, and the brief's cited PDF call;
/// it prints sizes, latencies and verdicts only.
#[tokio::test]
#[ignore = "calls the real Anthropic endpoint with the key file named by GENUI_BOX_SMOKE_KEY_FILE"]
async fn real_haiku_folder_smoke() {
    use crate::composer::model::{Anthropic, Model, DEFAULT_ENDPOINT, DEFAULT_ROUTER_MODEL};
    use serde_json::{json, Value};
    let Some(path) = std::env::var_os("GENUI_BOX_SMOKE_KEY_FILE") else {
        eprintln!("skipped: GENUI_BOX_SMOKE_KEY_FILE not set");
        return;
    };
    let text = std::fs::read_to_string(path).expect("the key file");
    let key = text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("ANTHROPIC_API_KEY=")
                .map(|k| k.trim().trim_matches('"').to_owned())
        })
        .expect("an ANTHROPIC_API_KEY line");
    let client = Anthropic::new(&key, DEFAULT_ENDPOINT).expect("a usable key");
    drop((text, key));
    let model: Arc<dyn Model> = Arc::new(client);
    let composer =
        crate::composer::Composer::new(Some(model), DEFAULT_ROUTER_MODEL, DEFAULT_ROUTER_MODEL);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/pitch/folder-fixture");
    let csv = std::fs::read_to_string(dir.join("q3_metrics.csv")).unwrap();
    let mut lines = csv
        .lines()
        .map(|l| l.split(',').map(str::to_owned).collect::<Vec<_>>());
    let table = super::inspect::Table {
        format: "csv",
        sheet: None,
        header: lines.next().unwrap(),
        rows: lines.collect(),
    };
    let columns = super::inspect::columns(&table);
    let source = "0a1b2c3d-0000-4000-8000-000000000001";
    let contract = super::register::contract("q3_metrics", source, &columns);
    let canonical = crate::admission::encode(&contract).unwrap();
    let data = crate::storage::FileData {
        source_id: source.into(),
        revision_id: "0a1b2c3d-0000-4000-8000-000000000002".into(),
        normalized_digest: format!("sha256:{}", "0".repeat(64)),
        row_count: table.rows.len() as i64,
        contract_digest: crate::admission::digest::digest(
            crate::admission::digest::INPUT_CONTRACT,
            &canonical,
        ),
        contract,
        created_at: "2026-10-30T00:00:00Z".into(),
    };
    let file = |name: &str, kind: &str| crate::storage::BoxFile {
        box_id: ID.into(),
        file_id: ID.into(),
        name: name.into(),
        kind: kind.into(),
        media_type: String::new(),
        bytes: 1,
        content_digest: String::new(),
        object_key: String::new(),
        created_at: String::new(),
        data: None,
    };
    let files = vec![
        file("q3_metrics.csv", "csv"),
        file("q3_metrics_brief.pdf", "pdf"),
    ];
    let derived = vec![super::derive::DataFile {
        file: files[0].clone(),
        data: data.clone(),
        sample: table.rows.iter().take(20).cloned().collect(),
    }];
    let context = super::derive::context("Q3 metrics", &files, &derived);
    let started = std::time::Instant::now();
    let generated = composer
        .generate_with(
            "Build an app that shows the key metrics",
            Some(&context),
            &[super::derive::readable(&data)],
            "00000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-00000000e0a1",
            tokio::time::Instant::now() + std::time::Duration::from_secs(100),
        )
        .await;
    println!(
        "{}",
        json!({"step": "generation", "context_bytes": context.to_string().len(), "ms": started.elapsed().as_millis() as u64,
            "admitted": generated.is_ok(), "refused_attempts": generated.as_ref().map(|g| g.refused.len()).unwrap_or(0),
            "views": generated.as_ref().map(|g| g.planned.admitted.view_count).unwrap_or(0)})
    );
    // The explanation over results the engine would compute (synthetic sums).
    let revenue: i64 = table
        .rows
        .iter()
        .map(|r| r[2].replace('.', "").parse::<i64>().unwrap())
        .sum();
    let signups: i64 = table
        .rows
        .iter()
        .map(|r| r[3].parse::<i64>().unwrap())
        .sum();
    let results = vec![super::explain::ViewResult {
        view_id: "kpis".into(),
        title: "Q3 totals".into(),
        columns: vec![
            "rows".into(),
            "total_revenue".into(),
            "total_signups".into(),
        ],
        column_types: json!({"rows": {"type": "integer"}, "total_revenue": {"type": "money", "currency": "USD", "minor_unit_exponent": 2}, "total_signups": {"type": "integer"}}),
        rows: vec![vec![
            json!(table.rows.len().to_string()),
            json!(revenue.to_string()),
            json!(signups.to_string()),
        ]],
        receipt_digest: "sha256:smoke".into(),
    }];
    let input = super::explain::model_input(&results);
    let started = std::time::Instant::now();
    let reply = composer
        .complete_fast(
            super::explain::SYSTEM,
            input.clone(),
            800,
            std::time::Duration::from_secs(20),
        )
        .await;
    let judged = reply
        .as_ref()
        .ok()
        .and_then(|r| super::explain::judge(&r.text, &results));
    // Verdict facts only (never the reply text): digits outside braces.
    let stray_digits = reply
        .as_ref()
        .map(|r| {
            let mut depth = 0i32;
            r.text
                .chars()
                .filter(|c| {
                    if *c == '{' {
                        depth += 1
                    } else if *c == '}' {
                        depth -= 1
                    };
                    depth <= 1 && c.is_ascii_digit()
                })
                .count()
        })
        .unwrap_or(0);
    println!(
        "{}",
        json!({"step": "explanation", "input_bytes": input.len(), "ms": started.elapsed().as_millis() as u64,
        "replied": reply.is_ok(), "accepted": judged.is_some(), "findings": judged.as_ref().map(Vec::len).unwrap_or(0),
        "digits_outside_placeholders": stray_digits})
    );
    if let Some(findings) = &judged {
        let filled = super::explain::fill(findings, &results).unwrap();
        let values: Vec<Value> = filled
            .iter()
            .flat_map(|f| {
                f["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|p| p.get("exact").cloned())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(values.iter().all(|v| [
            json!(table.rows.len().to_string()),
            json!(revenue.to_string()),
            json!(signups.to_string())
        ]
        .contains(v)));
    }
    // The brief's cited PDF call.
    let pdf = std::fs::read(dir.join("q3_metrics_brief.pdf")).unwrap();
    let started = std::time::Instant::now();
    let cited = composer
        .complete_documents(
            "You brief a busy owner on one document. Write one short paragraph (two to four sentences) of its key facts, citing the document for every factual statement. Plain prose only.",
            "Brief me on this document.".into(),
            &[crate::composer::model::DocumentInput { title: "q3_metrics_brief.pdf", bytes: &pdf }],
            700,
            std::time::Duration::from_secs(40),
        )
        .await;
    let parts = cited
        .as_ref()
        .map(|r| super::brief::cited_parts(r, ID))
        .unwrap_or_default();
    println!(
        "{}",
        json!({"step": "pdf_citations", "pdf_bytes": pdf.len(), "ms": started.elapsed().as_millis() as u64,
        "replied": cited.is_ok(), "input_tokens": cited.as_ref().ok().and_then(|r| r.input_tokens), "output_tokens": cited.as_ref().ok().and_then(|r| r.output_tokens),
        "cited_statements": parts.iter().filter(|p| p.get("cite").is_some()).count(), "kept_parts": parts.len()})
    );
}

// --- APP-124: start from your data ---

fn data_file(
    name: &str,
    columns: &[(&str, super::inspect::ColumnType)],
    rows: i64,
) -> super::derive::DataFile {
    use super::inspect::Column;
    let columns: Vec<Column> = columns
        .iter()
        .map(|(id, kind)| Column {
            id: (*id).into(),
            header: (*id).into(),
            kind: *kind,
            nullable: false,
        })
        .collect();
    let contract = super::register::contract(
        &super::register::input_id(name),
        "0a1b2c3d-0000-4000-8000-000000000000",
        &columns,
    );
    super::derive::DataFile {
        file: crate::storage::BoxFile {
            box_id: ID.into(),
            file_id: ID.into(),
            name: name.into(),
            kind: "csv".into(),
            media_type: "text/csv".into(),
            bytes: 10,
            content_digest: format!("sha256:{}", "a".repeat(64)),
            object_key: "k".into(),
            created_at: "2026-10-31T00:00:00Z".into(),
            data: None,
        },
        data: crate::storage::FileData {
            source_id: "0a1b2c3d-0000-4000-8000-000000000000".into(),
            revision_id: "0a1b2c3d-0000-4000-8000-000000000001".into(),
            normalized_digest: format!("sha256:{}", "b".repeat(64)),
            row_count: rows,
            contract,
            contract_digest: format!("sha256:{}", "c".repeat(64)),
            created_at: "2026-10-31T00:00:00Z".into(),
        },
        sample: Vec::new(),
    }
}

/// Make a card: exactly one card, the most useful view by rule (a chart of
/// a number over dates or categories, else the totals, else the rows),
/// admitted whole like any generated definition and runnable.
#[test]
fn make_a_card_is_one_admitted_view_chosen_by_rule() {
    use super::inspect::ColumnType::*;
    let org = "00000000-0000-4000-8000-0000000000c1";
    let app = "00000000-0000-4000-8000-0000000000c2";
    for (columns, kind) in [
        (
            vec![("month", Date), ("region", Text), ("revenue", Money)],
            "chart",
        ),
        (vec![("region", Text), ("revenue", Money)], "chart"),
        (vec![("revenue", Money), ("signups", Integer)], "summary"),
        (vec![("region", Text), ("owner", Text)], "summary"),
    ] {
        let file = data_file("Q3 metrics.csv", &columns, 12);
        let content = super::derive::card_definition("Q3 board pack", &file);
        let views = content["views"].as_array().unwrap();
        assert_eq!(views.len(), 1, "{columns:?}");
        assert_eq!(views[0]["kind"], kind, "{columns:?}");
        assert_eq!(content["default_layout"].as_array().unwrap().len(), 1);
        assert_eq!(content["title"], "Q3 board pack");
        assert!(content["record_schemas"].as_array().unwrap().is_empty());
        assert!(content["commands"].as_array().unwrap().is_empty());
        crate::composer::plan::admit(&content, org, app).expect("admitted");
        crate::composer::plan::require_runnable(&content).expect("runnable");
    }
    // Named after the folder; after the file when the folder has no name of its own.
    assert_eq!(
        super::derive::card_title("Q3 board pack", "box", "q3.csv"),
        "Q3 board pack"
    );
    assert_eq!(
        super::derive::card_title("Untitled folder", "box", "Q3 metrics.csv"),
        "Q3 metrics"
    );
    assert_eq!(
        super::derive::card_title("Uploads", "uploads", "spend.xlsx"),
        "spend"
    );
    for (title, slug) in [
        ("Q3 board pack", "q3-board-pack"),
        ("  Spend: 2026!  ", "spend-2026"),
        ("2026 sales", "data-2026-sales"),
        ("Ünïcode only", "unicode-only"),
        ("Ventas año", "ventas-ano"),
        (&"long name ".repeat(6), "long-name-long-name-long-na"),
    ] {
        let made = super::derive::card_slug(title);
        assert_eq!(made, slug, "{title}");
        assert!(crate::composer::commands::valid_slug(&made), "{made}");
        assert!(crate::composer::commands::valid_slug(&format!(
            "{made}-card"
        )));
    }
}

fn folder_table(rows: &[[&str; 3]], total: usize) -> super::ask::FolderTable {
    use super::inspect::ColumnType::*;
    super::ask::FolderTable {
        file_id: ID.into(),
        name: "q3_metrics.csv".into(),
        sha256: format!("sha256:{}", "d".repeat(64)),
        sheet: None,
        columns: vec![
            ("month".into(), Date),
            ("region".into(), Text),
            ("revenue".into(), Money),
        ],
        rows: rows
            .iter()
            .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
            .collect(),
        total_rows: total,
    }
}

/// Ask on a folder: the body is exactly `{"question"}`; a rehearsed count or
/// total references every row it used and adds money exactly; a partial
/// read says so; the answer names each row's file and row, from the server.
#[test]
fn folder_ask_answers_with_the_rows_it_used() {
    use super::ask::{document, question_of, rehearsed, user};
    use serde_json::json;
    let object = |v: serde_json::Value| v.as_object().unwrap().clone();
    assert_eq!(
        question_of(&object(json!({"question": " how many? "}))).as_deref(),
        Some("how many?")
    );
    for refused in [
        json!({}),
        json!({"question": ""}),
        json!({"question": 1}),
        json!({"question": "x", "scope": {}}),
        json!({"question": "a\u{7}b"}),
    ] {
        assert!(question_of(&object(refused.clone())).is_none(), "{refused}");
    }
    let rows = [
        ["2026-07-01", "North", "214215.91"],
        ["2026-07-01", "South", "353481.49"],
        ["2026-08-01", "East", "12"],
    ];
    let tables = vec![folder_table(&rows, 3)];
    let count = rehearsed("how many rows are there?", &tables).unwrap();
    assert_eq!(count.text, "q3_metrics.csv has 3 rows.");
    assert_eq!(count.references, ["f1r1", "f1r2", "f1r3"]);
    let total = rehearsed("what's the total revenue", &tables).unwrap();
    assert_eq!(
        total.text,
        "revenue in q3_metrics.csv adds up to $567,709.40 (3 rows)."
    );
    assert_eq!(total.references.len(), 3);
    let partial = vec![folder_table(&rows, 450)];
    assert!(rehearsed("how many", &partial)
        .unwrap()
        .text
        .contains("at least 3 rows"));
    assert!(rehearsed("sum it", &partial)
        .unwrap()
        .text
        .contains("not the full total"));
    let change = rehearsed("make it a chart", &tables).unwrap();
    assert!(change.text.starts_with("Ask only reads") && change.references.is_empty());
    assert!(
        rehearsed("why did the south grow?", &tables).is_none(),
        "reasoning goes to the model"
    );
    // The document: exactly {text, references}; each reference the file, its digest and the row.
    let answer = crate::composer::ask::Answer {
        text: "South is largest.".into(),
        references: vec!["f1r2".into()],
    };
    let doc = document(&answer, &tables);
    assert_eq!(
        doc.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["references", "text"]
    );
    assert_eq!(
        doc["references"],
        json!([{"ref": "f1r2", "file_id": ID, "file": "q3_metrics.csv", "sha256": format!("sha256:{}", "d".repeat(64)), "sheet": null, "row": 2, "label": "2026-07-01 · South · 353481.49"}])
    );
    // The model sees the rows by reference and may cite only those.
    let (message, known) = user("which region is largest?", &tables);
    assert!(message.contains("\"f1r3\"") && !message.contains("\"f1r4\""));
    assert_eq!(known.len(), 3);
    assert!(
        crate::composer::ask::judge(r#"{"text":"South.","references":["f1r2"]}"#, &known).is_ok()
    );
    assert_eq!(
        crate::composer::ask::judge(r#"{"text":"South.","references":["f1r9"]}"#, &known),
        Err("unknown_reference")
    );
    assert_eq!(
        crate::composer::ask::judge(r#"{"text":"x","chart":"bar"}"#, &known),
        Err("view_spec")
    );
    // A long table is cut to the model's byte cap and marked partial.
    let long: Vec<[&str; 3]> = (0..200)
        .map(|_| ["2026-07-01", "a very long region name", "1.00"])
        .collect();
    let (message, known) = user("q", &[folder_table(&long, 200)]);
    assert!(known.len() <= 200 && message.len() <= 60_000);
}

/// APP-124 review: one address function for the server's judgment and the
/// canvas's folder addresses (`boxes.ts` `folderSlug`, the same vectors in
/// `canvas-cards.test.ts`): NFKD, marks dropped, 64 characters; so a card
/// named "Ventas ano" is judged against the folder "Ventas año" the canvas
/// answers to as `/ventas-ano`. The names a card is offered, in order.
#[test]
fn card_addresses_are_judged_as_the_canvas_addresses_folders() {
    use super::derive::{address_slug, card_slug};
    for (label, slug) in [
        ("Ventas año", "ventas-ano"),
        ("Ventas ano", "ventas-ano"),
        ("Ünïcode only", "unicode-only"),
        ("  Spend: 2026!  ", "spend-2026"),
        ("ﬁnance Ⅳ", "finance-iv"),
        (
            &"long name ".repeat(8),
            "long-name-long-name-long-name-long-name-long-name-long-name-long",
        ),
    ] {
        assert_eq!(address_slug(label), slug, "{label}");
    }
    assert_eq!(card_slug("Ventas ano"), address_slug("Ventas año"));
    // A folder address longer than a card's base is compared whole.
    let folder = "abcdefghij abcdefghij abcde card";
    assert_eq!(address_slug(folder), "abcdefghij-abcdefghij-abcde-card");
    let base = card_slug("abcdefghij abcdefghij abcdefghij");
    assert_eq!(base, "abcdefghij-abcdefghij-abcde");
    assert_eq!(
        super::routes::card_addresses(&base)[1],
        address_slug(folder),
        "the -card name is the folder's own address: judged taken"
    );
    assert_eq!(
        super::routes::card_addresses("q3"),
        ["q3", "q3-card", "q3-2", "q3-3", "q3-4", "q3-5", "q3-6", "q3-7", "q3-8", "q3-9"]
    );
}

/// APP-124 review: a rehearsed total skips a cell that is not a number and
/// says how many it skipped (never no answer); Ask with no readable table is
/// its text answer, not a 503.
#[test]
fn folder_ask_total_skips_cells_that_are_not_numbers() {
    use super::ask::{no_table, rehearsed};
    let rows = [
        ["2026-07-01", "North", "214215.91"],
        ["2026-07-01", "South", "n/a"],
        ["2026-08-01", "East", ""],
        ["2026-08-01", "West", "12"],
    ];
    let total = rehearsed("what's the total revenue", &[folder_table(&rows, 4)]).unwrap();
    assert_eq!(
        total.text,
        "revenue in q3_metrics.csv adds up to $214,227.91 (2 rows; 1 cell that is not a number was skipped)."
    );
    assert_eq!(total.references, ["f1r1", "f1r4"]);
    let rows = [["2026-07-01", "North", "x"], ["2026-07-01", "South", "y"]];
    let none = rehearsed("sum", &[folder_table(&rows, 2)]).unwrap();
    assert!(none
        .text
        .contains("$0.00 (0 rows; 2 cells that are not numbers were skipped)"));
    assert!(none.references.is_empty());
    assert!(no_table(0)
        .text
        .contains("no table (CSV or XLSX) to read yet"));
    assert!(no_table(1)
        .text
        .contains("one table (CSV or XLSX) could not be read"));
    assert!(no_table(3)
        .text
        .starts_with("None of this folder's 3 tables"));
    assert!(no_table(2).references.is_empty());
}

/// ADR-011 I4 (APP-124 review): a folder build's idempotency key answers the
/// first result for the same member, folder and body; another body is
/// refused; another scope is its own key.
#[tokio::test]
async fn a_folder_build_key_answers_its_first_result() {
    use super::routes::replay_claim;
    let scope = |n: u8| format!("org:member:box:{n:032x}-{}", std::process::id());
    let first = replay_claim(scope(1), r#"{"one_card":true}"#.into()).unwrap();
    *first.lock().await = Some(serde_json::json!({"app_id": "a"}));
    let again = replay_claim(scope(1), r#"{"one_card":true}"#.into()).unwrap();
    assert_eq!(
        again.lock().await.as_ref().unwrap()["app_id"],
        "a",
        "a repeat answers the first result"
    );
    assert!(
        replay_claim(scope(1), r#"{"purpose":"x"}"#.into()).is_none(),
        "the same key with another body is refused"
    );
    let other = replay_claim(scope(2), r#"{"one_card":true}"#.into()).unwrap();
    assert!(other.lock().await.is_none(), "another key runs");
    // A first request that failed leaves nothing: the repeat runs.
    let failed = replay_claim(scope(3), "{}".into()).unwrap();
    drop(failed.lock().await);
    assert!(replay_claim(scope(3), "{}".into())
        .unwrap()
        .lock()
        .await
        .is_none());
}
