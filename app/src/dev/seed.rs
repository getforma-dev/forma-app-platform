//! The pitch profile's example data, once per database (ADR-012 D12.1),
//! made through the product's own routes in process: Diego's expense claims
//! (one approved by Sarah, one waiting for her), his time off waiting for
//! her, and his "Q3 board pack" folder with `docs/pitch/folder-fixture`.
use super::repository;
use crate::auth::cookies;
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

const FOLDER_FIXTURE: &str = "docs/pitch/folder-fixture";
const SAME_ORIGIN: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/json"),
];
/// Sarah's principal (the approver when a draft names none).
const SARAH: &str = super::SARAH;

struct Reply {
    status: StatusCode,
    body: String,
}
impl Reply {
    fn json(&self) -> Result<Value, String> {
        serde_json::from_str(&self.body).map_err(|_| format!("not JSON: {}", self.body))
    }
    fn expect(self, status: StatusCode, label: &str) -> Result<Self, String> {
        if self.status == status {
            Ok(self)
        } else {
            Err(format!("{label}: {} {}", self.status, self.body))
        }
    }
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    headers: &[(&str, &str)],
    body: Body,
) -> Result<Reply, String> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION));
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut request = request.body(body).map_err(|e| e.to_string())?;
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:4000"
            .parse::<std::net::SocketAddr>()
            .expect("address"),
    ));
    let response = match app.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 22)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Reply {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

async fn act(app: &Router, cookie: &str, body: Value) -> Result<Value, String> {
    send(
        app,
        "POST",
        crate::actions::PATH,
        cookie,
        &SAME_ORIGIN,
        Body::from(body.to_string()),
    )
    .await?
    .expect(StatusCode::OK, &body.to_string())?
    .json()
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("{key} missing in {value}"))
}

async fn confirm(app: &Router, cookie: &str, draft: &Value) -> Result<String, String> {
    let request = &draft["command_request"];
    let key = text(request, "idempotency_key")?;
    let uri = format!(
        "/v1/work/{}/{}/commands/{}",
        text(draft, "app_id")?,
        text(draft, "work_id")?,
        text(draft, "command_id")?
    );
    let headers = [
        ("sec-fetch-site", "same-origin"),
        ("content-type", "application/json"),
        ("idempotency-key", key),
    ];
    let reply = send(
        app,
        "POST",
        &uri,
        cookie,
        &headers,
        Body::from(request.to_string()),
    )
    .await?
    .expect(StatusCode::OK, &uri)?
    .json()?;
    Ok(text(&reply, "record_id")?.to_owned())
}

async fn share(app: &Router, cookie: &str, draft: &Value) -> Result<(), String> {
    let uri = format!(
        "/v1/work/{}/{}/share",
        text(draft, "app_id")?,
        text(draft, "work_id")?
    );
    let mut approvers: Vec<String> = draft["approvers"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| a["principal_id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if approvers.is_empty() {
        approvers.push(SARAH.to_owned());
    }
    for principal in approvers {
        let body = json!({"grantee_principal_id": principal, "record_write": true}).to_string();
        // A work the filer did not create (the library's seeded work, which
        // every persona holds by `grant-library-access`) is not his to share:
        // the approver already reads it.
        let reply = send(app, "POST", &uri, cookie, &SAME_ORIGIN, Body::from(body)).await?;
        if reply.status != StatusCode::OK && reply.status != StatusCode::NOT_FOUND {
            return Err(format!("{uri}: {} {}", reply.status, reply.body));
        }
    }
    Ok(())
}

async fn file_and_share(
    app: &Router,
    diego: &str,
    sentence: &str,
) -> Result<(Value, String), String> {
    let draft = act(app, diego, json!({"utterance": sentence})).await?;
    let record = confirm(app, diego, &draft).await?;
    share(app, diego, &draft).await?;
    Ok((draft, record))
}

pub(crate) async fn examples(app: &Router, diego: &str, sarah: &str) -> Result<(), String> {
    let (lunch, lunch_record) = file_and_share(
        app,
        diego,
        "I spent $42 on a team lunch at Sweetgreen yesterday, Sarah approves my expenses",
    )
    .await?;
    let decision = act(
        app,
        sarah,
        json!({"decide": {"app_id": lunch["app_id"], "work_id": lunch["work_id"], "record_id": lunch_record, "status": "approved"}}),
    )
    .await?;
    confirm(app, sarah, &decision).await?;
    file_and_share(
        app,
        diego,
        "I spent $389 on a flight at Delta yesterday, Sarah approves my expenses",
    )
    .await?;
    file_and_share(
        app,
        diego,
        "Book me off Nov 24–26 for Thanksgiving, Sarah approves",
    )
    .await?;
    println!("genui-dev: records: one approved expense, an expense and a time off request waiting for Sarah");

    let created = send(
        app,
        "POST",
        "/v1/boxes",
        diego,
        &SAME_ORIGIN,
        Body::from(json!({"name": "Q3 board pack"}).to_string()),
    )
    .await?
    .expect(StatusCode::CREATED, "create the folder")?
    .json()?;
    let box_id = text(&created["box"], "box_id")?.to_owned();
    let fixture = repository().join(FOLDER_FIXTURE);
    let boundary = "----genuidevboundary";
    for name in ["q3_metrics.csv", "q3_pipeline.csv", "q3_metrics_brief.pdf"] {
        let bytes = std::fs::read(fixture.join(name)).map_err(|e| format!("{name}: {e}"))?;
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
        body.extend_from_slice(&bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let content_type = format!("multipart/form-data; boundary={boundary}");
        let reply = send(
            app,
            "POST",
            &format!("/v1/boxes/{box_id}/files"),
            diego,
            &[
                ("sec-fetch-site", "same-origin"),
                ("content-type", &content_type),
            ],
            Body::from(body),
        )
        .await?;
        if !reply.status.is_success() {
            return Err(format!("upload {name}: {} {}", reply.status, reply.body));
        }
    }
    println!("genui-dev: folder \"Q3 board pack\" (Diego) with {FOLDER_FIXTURE}");
    Ok(())
}
