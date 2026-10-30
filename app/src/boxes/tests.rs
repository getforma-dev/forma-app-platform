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
