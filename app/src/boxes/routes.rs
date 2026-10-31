//! APP-108 routes. Every route needs a live workspace session; storage
//! decides (`storage::boxes`), so an absent, foreign or unreadable box, file
//! or work is the uniform 404, byte for byte. Mutations are same-origin
//! (403 `cross_site` otherwise) and rate-limited per member.
//!
//! - `GET /v1/boxes`, `POST /v1/boxes {"name","app_id"?}`, `POST
//!   /v1/boxes/uploads {}` (the member's implicit "Uploads" box).
//! - `GET /v1/boxes/{box}`: the box, the files this member may read and
//!   (owner or shared) the apps built from it.
//! - `POST /v1/boxes/{box}/files`: `multipart/form-data`, one part `file`.
//!   Refusal order: cross-site 403, session 404, rate 429, body over 20 MB +
//!   framing 413 `file_too_large`, framing 422 `unsupported_file`, the box
//!   (owner, uniform 404), room 409 `box_full`, type and magic bytes 422
//!   `unsupported_file`; only then is the object written and the row
//!   recorded (audited). A CSV/XLSX is parsed at once by the sealed parser
//!   and the answer carries its typed preview.
//! - `GET /v1/boxes/{box}/files/{file}/content`: the bytes, same-origin only,
//!   `Content-Disposition: inline`, `Content-Security-Policy: sandbox`,
//!   `X-Content-Type-Options: nosniff`, `Cache-Control: private, no-store`.
//! - `GET /v1/boxes/{box}/files/{file}/preview`: the typed preview.
//! - `POST /v1/boxes/{box}/files/{file}/data {"columns","card"}`: confirm the
//!   types → a registered source revision (and, with `card`, a data card).
//! - `POST /v1/boxes/{box}/files/{file}/attach {"app_id","work_id"}`: the file
//!   into the member's box attached to that app, plus its data card.
//! - `POST /v1/boxes/{box}/apps {"purpose","file_ids"?}`: an app built from
//!   the box (long deadline: it may wait on the model). APP-124: with
//!   `"one_card": true` (`purpose` optional) it is **Make a card**: a
//!   minimal app with exactly one card, by rule ([`card_from_box`]). An
//!   `Idempotency-Key` (32 lowercase hex digits; malformed is 400) makes
//!   either idempotent (ADR-011 I4): a repeat by the member on the folder
//!   answers the first result (201, `Idempotent-Replayed: true`), the same
//!   key with another body is 422 `idempotency_key_reused`.
//! - `POST /v1/boxes/{box}/ask {"question"}`: APP-124, Ask on a folder
//!   (`super::ask`): a read-only answer over its parsed tables, with the
//!   file and row of every row it used.
//! - `POST /v1/boxes/{box}/share {"grantee_principal_id"}`.
//! - `GET /v1/boxes/lineage/{app}/{work}`: a work's provenance.
//! - `GET|POST /v1/boxes/explain/{app}/{work}`: the explanation card.
use super::{
    derive::{self, DataFile},
    explain,
    inspect::{self, Column, Table},
    register::{self, Normalizer, RegisterError},
    sniff,
    store::{self, ObjectStore},
};
use crate::{
    auth::{csrf, Auth},
    composer::{self, routes::Refused, Composer},
    http::{self, limits, UniformPath},
    imports::sandbox::ParserIdentity,
    query::Work,
    storage::{self, BoxFile, BoxRow, Derivation, Error, FileData, TrustedRequest},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

pub const UPLOAD_MEMBER: &[limits::Window] = &[limits::per(120, 3600)];
pub const EXPLAIN_MEMBER: &[limits::Window] = &[limits::per(30, 3600)];
/// The long-deadline paths (`http::uniform_with_many`).
pub const APPS_PATH: &str = "/v1/boxes/{box}/apps";
pub const EXPLAIN_PATH: &str = "/v1/boxes/explain/{app}/{work}";
pub const BRIEF_PATH: &str = "/v1/boxes/{box}/brief";
const EXPLAIN_BUDGET: Duration = Duration::from_secs(20);

/// The routes' state.
pub struct Boxes {
    pub auth: Arc<Auth>,
    pub work: Arc<Work>,
    pub composer: Composer,
    pub store: ObjectStore,
    pub normalizer: Normalizer,
    /// The sealed parser's verified identity (none: no CSV/XLSX is parsed).
    pub parser: Option<ParserIdentity>,
}

/// The short-deadline routes.
pub fn router(state: Arc<Boxes>) -> Router {
    Router::new()
        .route("/v1/boxes", get(list).post(create))
        .route("/v1/boxes/uploads", post(uploads))
        .route("/v1/boxes/{box}", get(read))
        .route("/v1/boxes/{box}/share", post(share))
        .route("/v1/boxes/{box}/name", post(rename))
        .route("/v1/boxes/lineage/{app}/{work}", get(lineage))
        .route("/v1/boxes/{box}/files/{file}/content", get(content))
        // Long deadline (`long_path`).
        .route("/v1/boxes/{box}/files", post(upload))
        .route("/v1/boxes/{box}/files/{file}/preview", get(preview))
        .route("/v1/boxes/{box}/files/{file}/data", post(data))
        .route("/v1/boxes/{box}/files/{file}/attach", post(attach))
        .route(APPS_PATH, post(apps))
        .route(EXPLAIN_PATH, get(explain_get).post(explain_post))
        .route(
            BRIEF_PATH,
            get(super::brief::brief_get).post(super::brief::brief_post),
        )
        .route(super::ask::ASK_PATH, post(super::ask::ask_post))
        .with_state(state)
}
/// Whether `path` is a route that runs the parser, the normalizer or a
/// model: `http::deadline` gives it the composer's deadline
/// (`COMPOSE_DEADLINE`) instead of the D7.14 handler deadline.
/// (`http::uniform_with_many` routes only static paths; these carry ids.)
pub fn long_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/v1/boxes/") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    matches!(
        parts.as_slice(),
        [_, "files" | "apps" | "brief" | "ask"]
            | [_, "files", _, "preview" | "data" | "attach"]
            | ["explain", _, _]
    )
}

pub(crate) fn document(status: StatusCode, value: &Value) -> Response {
    let Ok(bytes) = crate::admission::encode(value) else {
        return http::unavailable();
    };
    let mut response = (status, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}
fn refusal(error: Error) -> Response {
    tracing::info!(code = %error, "box.refused");
    match error {
        Error::Denied => http::not_found(),
        _ => http::unavailable(),
    }
}

pub(crate) async fn session(state: &Boxes, headers: &HeaderMap) -> Option<TrustedRequest> {
    crate::sharing::session(&state.auth, headers)
        .await
        .map(|(_, member)| member)
}

pub fn box_json(row: &BoxRow, number: usize) -> Value {
    json!({"box_id": row.box_id, "number": number, "name": row.name, "kind": row.kind,
        "app_id": row.app_id, "owner": row.owner, "file_count": row.file_count, "created_at": row.created_at})
}
pub fn data_json(data: &FileData) -> Value {
    json!({"source_id": data.source_id, "revision_id": data.revision_id,
        "normalized_digest": data.normalized_digest, "row_count": data.row_count,
        "fields": data.contract.get("fields").cloned().unwrap_or(Value::Null)})
}
pub fn file_json(file: &BoxFile) -> Value {
    json!({"file_id": file.file_id, "name": file.name, "kind": file.kind, "media_type": file.media_type,
        "bytes": file.bytes, "sha256": file.content_digest, "created_at": file.created_at,
        "content_url": format!("/v1/boxes/{}/files/{}/content", file.box_id, file.file_id),
        "data": file.data.as_ref().map(data_json)})
}
fn derivation_json(d: &Derivation, title: &str) -> Value {
    json!({"app_id": d.app_id, "work_id": d.work_id, "title": title, "kind": d.kind,
        "file_ids": d.file_ids, "created_at": d.created_at})
}
fn json_body(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
}
/// A JSON object body (`malformed_request` for anything else).
pub(crate) async fn json_of(
    body: axum::body::Body,
    headers: &HeaderMap,
) -> Result<serde_json::Map<String, Value>, Response> {
    let bytes = http::bounded(body, http::JSON_BODY_LIMIT)
        .await
        .ok_or_else(http::payload_too_large)?;
    if !json_body(headers) {
        return Err(http::problem(StatusCode::BAD_REQUEST, "malformed_request"));
    }
    match crate::admission::decode(&bytes) {
        Ok(Value::Object(object)) => Ok(object),
        _ => Err(http::problem(StatusCode::BAD_REQUEST, "malformed_request")),
    }
}
/// The member's `MEMBER_WRITES` window (folder create, uploads, rename, share).
fn admit_write(state: &Boxes, member: &TrustedRequest) -> Result<(), u64> {
    state
        .auth
        .limits
        .admit_member_write(member.organization(), member.principal())
}
pub(crate) fn valid(id: &str) -> bool {
    storage::valid_identifier(id)
}

/// The member's boxes, numbered from 1 in creation order.
pub async fn numbered(
    state: &Boxes,
    member: &TrustedRequest,
) -> Result<Vec<(usize, BoxRow)>, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    Ok(stores
        .workspace
        .list_boxes(member)
        .await?
        .into_iter()
        .enumerate()
        .map(|(i, row)| (i + 1, row))
        .collect())
}

async fn list(State(state): State<Arc<Boxes>>, request: Request) -> Response {
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    match numbered(&state, &member).await {
        Ok(rows) => document(
            StatusCode::OK,
            &json!({"noun": super::NOUN, "boxes": rows.iter().map(|(n, row)| box_json(row, *n)).collect::<Vec<_>>()}),
        ),
        Err(error) => refusal(error),
    }
}

async fn number_of(state: &Boxes, member: &TrustedRequest, box_id: &str) -> usize {
    numbered(state, member)
        .await
        .ok()
        .and_then(|rows| {
            rows.iter()
                .find(|(_, r)| r.box_id == box_id)
                .map(|(n, _)| *n)
        })
        .unwrap_or(0)
}

async fn create(State(state): State<Arc<Boxes>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry) = admit_write(&state, &member) {
        return http::too_many_requests(retry);
    }
    let object = match json_of(body, &parts.headers).await {
        Ok(object) => object,
        Err(response) => return response,
    };
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .and_then(storage::box_name);
    let app = match object.get("app_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(app)) if valid(app) => Some(app.clone()),
        Some(_) => return http::problem(StatusCode::BAD_REQUEST, "malformed_request"),
    };
    let (Some(name), true) = (name, object.keys().all(|k| k == "name" || k == "app_id")) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_request");
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    let kind = if app.is_some() { "app" } else { "box" };
    match stores
        .workspace
        .create_box(&member, &name, kind, app.as_deref())
        .await
    {
        Ok(row) => {
            let number = number_of(&state, &member, &row.box_id).await;
            document(StatusCode::CREATED, &json!({"box": box_json(&row, number)}))
        }
        Err(error) => refusal(error),
    }
}

async fn uploads(State(state): State<Arc<Boxes>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry) = admit_write(&state, &member) {
        return http::too_many_requests(retry);
    }
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    match stores
        .workspace
        .create_box(&member, "Uploads", "uploads", None)
        .await
    {
        Ok(row) => {
            let number = number_of(&state, &member, &row.box_id).await;
            document(StatusCode::OK, &json!({"box": box_json(&row, number)}))
        }
        Err(error) => refusal(error),
    }
}

/// The titles of works the member can open (for "Apps built from this box").
async fn titles(state: &Boxes, member: &TrustedRequest, apps: &[Derivation]) -> Vec<Value> {
    let Some(stores) = &state.auth.stores else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for d in apps {
        if let Ok(outline) = stores
            .data
            .work_outline(member, &d.app_id, &d.work_id)
            .await
        {
            out.push(derivation_json(d, &outline.title));
        }
    }
    out
}

pub async fn box_view_json(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
) -> Result<Value, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    let view = stores.workspace.read_box(member, box_id).await?;
    let number = number_of(state, member, box_id).await;
    let apps = titles(state, member, &view.apps).await;
    Ok(json!({"box": box_json(&view.row, number),
        "files": view.files.iter().map(file_json).collect::<Vec<_>>(), "apps": apps}))
}

async fn read(
    State(state): State<Arc<Boxes>>,
    UniformPath(box_id): UniformPath<String>,
    request: Request,
) -> Response {
    if !valid(&box_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    match box_view_json(&state, &member, &box_id).await {
        Ok(value) => document(StatusCode::OK, &value),
        Err(error) => refusal(error),
    }
}

/// The one `file` part of a `multipart/form-data` body: `(filename, bytes)`.
pub fn file_part<'a>(content_type: &str, body: &'a [u8]) -> Option<(String, &'a [u8])> {
    let boundary = crate::actions::receipt::boundary(content_type)?;
    let delimiter = format!("--{boundary}");
    let separator = format!("\r\n--{boundary}");
    let find = |haystack: &[u8], needle: &[u8], from: usize| -> Option<usize> {
        haystack
            .get(from..)?
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + from)
    };
    if !body.starts_with(delimiter.as_bytes()) {
        return None;
    }
    let mut at = delimiter.len();
    let mut found: Option<(String, &[u8])> = None;
    loop {
        if body.get(at..at + 2)? == b"--" {
            break;
        }
        if body.get(at..at + 2)? != b"\r\n" {
            return None;
        }
        at += 2;
        let headers_end = find(body, b"\r\n\r\n", at)?;
        let headers = std::str::from_utf8(&body[at..headers_end]).ok()?;
        let content_start = headers_end + 4;
        let content_end = find(body, separator.as_bytes(), content_start)?;
        let disposition = headers.split("\r\n").find(|line| {
            line.to_ascii_lowercase()
                .starts_with("content-disposition:")
        })?;
        let field = |key: &str| -> Option<String> {
            disposition.split(';').find_map(|piece| {
                let (k, v) = piece.trim().split_once('=')?;
                (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().trim_matches('"').to_owned())
            })
        };
        if field("name").as_deref() == Some("file") {
            if found.is_some() {
                return None;
            }
            found = Some((field("filename")?, &body[content_start..content_end]));
        } else {
            return None;
        }
        at = content_end + separator.len();
    }
    found
}

/// Parses a stored data file and infers its columns (a parse refusal's code).
pub async fn inspect_file(
    state: &Boxes,
    member: &TrustedRequest,
    file: &BoxFile,
) -> Result<(Vec<u8>, Table, Vec<Column>), String> {
    let identity = state.parser.as_ref().ok_or("parser_unavailable")?;
    let bytes = state
        .store
        .get(
            member.organization(),
            &file.object_key,
            &file.content_digest,
        )
        .await
        .map_err(|_| "object_unavailable".to_owned())?;
    let table = inspect::parse(identity, &bytes, &file.kind).await?;
    if table.rows.is_empty() {
        return Err("no_rows".into());
    }
    let columns = inspect::columns(&table);
    Ok((bytes, table, columns))
}

fn preview_json(result: &Result<(Vec<u8>, Table, Vec<Column>), String>) -> Value {
    match result {
        Ok((_, table, columns)) => inspect::preview(table, columns),
        Err(code) => json!({"refused": code}),
    }
}

async fn upload(
    State(state): State<Arc<Boxes>>,
    UniformPath(box_id): UniformPath<String>,
    request: Request,
) -> Response {
    let started = Instant::now();
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    let key = format!(
        "box-upload:{}:{}",
        member.organization(),
        member.principal()
    );
    if let Err(retry) = state.auth.limits.admit(&[(&key, UPLOAD_MEMBER)]) {
        return http::too_many_requests(retry);
    }
    let Some(bytes) = http::bounded(body, super::UPLOAD_BODY_LIMIT).await else {
        return http::problem(StatusCode::PAYLOAD_TOO_LARGE, "file_too_large");
    };
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some((raw_name, file)) = file_part(content_type, &bytes) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_file");
    };
    if file.len() > super::MAX_FILE_BYTES {
        return http::problem(StatusCode::PAYLOAD_TOO_LARGE, "file_too_large");
    }
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    match stores.workspace.box_has_room(&member, &box_id).await {
        Ok(true) => {}
        Ok(false) => return http::problem(StatusCode::CONFLICT, "box_full"),
        Err(error) => return refusal(error),
    }
    let (Some(name), kind) = (sniff::file_name(&raw_name), sniff::sniff(&raw_name, file)) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_file");
    };
    let Some(kind) = kind else {
        tracing::info!(
            event = "box.upload_refused",
            code = "unsupported_file",
            bytes = file.len(),
            "upload refused"
        );
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unsupported_file");
    };
    let Some(file_id) = crate::auth::capability::random_uuid() else {
        return http::unavailable();
    };
    let organization = member.organization().to_owned();
    let object_key = store::file_key(&organization, &box_id, &file_id);
    let digest = store::sha256(file);
    if state
        .store
        .put(&organization, &object_key, file)
        .await
        .is_err()
    {
        return http::unavailable();
    }
    let stored = stores
        .workspace
        .add_box_file(
            &member,
            &box_id,
            &storage::NewFile {
                file_id: &file_id,
                name: &name,
                kind: kind.kind,
                media_type: kind.media_type,
                bytes: file.len() as i64,
                content_digest: &digest,
                object_key: &object_key,
            },
        )
        .await;
    // Review: an object whose row was definitely not written is discarded
    // (an unknown outcome keeps it: the row may exist).
    let row = match stored {
        Ok(Some(row)) => row,
        Ok(None) => {
            state.store.discard(&organization, &object_key).await;
            return http::problem(StatusCode::CONFLICT, "box_full");
        }
        Err(error) => {
            if error == storage::Error::Denied {
                state.store.discard(&organization, &object_key).await;
            }
            return refusal(error);
        }
    };
    let stored_ms = started.elapsed().as_millis() as u64;
    let preview = if matches!(row.kind.as_str(), "csv" | "xlsx") {
        preview_json(&inspect_file(&state, &member, &row).await)
    } else {
        Value::Null
    };
    tracing::info!(
        event = "box.file_added",
        organization_id = member.organization(),
        box_id,
        file_id,
        kind = kind.kind,
        bytes = file.len(),
        stored_ms,
        total_ms = started.elapsed().as_millis() as u64,
        "box file added"
    );
    document(
        StatusCode::CREATED,
        &json!({"file": file_json(&row), "preview": preview,
            "timings": {"store_ms": stored_ms, "parse_ms": started.elapsed().as_millis() as u64 - stored_ms}}),
    )
}

async fn content(
    State(state): State<Arc<Boxes>>,
    UniformPath((box_id, file_id)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !valid(&box_id) || !valid(&file_id) {
        return http::not_found();
    }
    // Same-origin fetches and frames only; a cross-site embed gets nothing.
    let site = request
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok());
    if matches!(site, Some(s) if s != "same-origin" && s != "none") {
        return http::not_found();
    }
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    let file = match stores
        .workspace
        .read_box_file(&member, &box_id, &file_id)
        .await
    {
        Ok(file) => file,
        Err(error) => return refusal(error),
    };
    let Ok(bytes) = state
        .store
        .get(
            member.organization(),
            &file.object_key,
            &file.content_digest,
        )
        .await
    else {
        return http::unavailable();
    };
    let media = match file.kind.as_str() {
        "csv" | "text" => "text/plain; charset=utf-8",
        _ => file.media_type.as_str(),
    };
    let ascii: String = file
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._- ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut response = (StatusCode::OK, bytes).into_response();
    let headers = response.headers_mut();
    let set = |headers: &mut HeaderMap, name: header::HeaderName, value: &str| {
        if let Ok(value) = HeaderValue::from_str(value) {
            headers.insert(name, value);
        }
    };
    set(headers, header::CONTENT_TYPE, media);
    set(
        headers,
        header::CONTENT_DISPOSITION,
        &format!("inline; filename=\"{ascii}\""),
    );
    set(
        headers,
        header::CONTENT_SECURITY_POLICY,
        "sandbox; frame-ancestors 'self'",
    );
    set(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(headers, header::CACHE_CONTROL, "private, no-store");
    set(headers, header::REFERRER_POLICY, "no-referrer");
    set(
        headers,
        header::HeaderName::from_static("cross-origin-resource-policy"),
        "same-origin",
    );
    response
}

async fn preview(
    State(state): State<Arc<Boxes>>,
    UniformPath((box_id, file_id)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !valid(&box_id) || !valid(&file_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    let file = match stores
        .workspace
        .read_box_file(&member, &box_id, &file_id)
        .await
    {
        Ok(file) if matches!(file.kind.as_str(), "csv" | "xlsx") => file,
        Ok(_) => return http::not_found(),
        Err(error) => return refusal(error),
    };
    let mut value = preview_json(&inspect_file(&state, &member, &file).await);
    if let (Some(object), Some(data)) = (value.as_object_mut(), &file.data) {
        object.insert("data".into(), data_json(data));
    }
    document(StatusCode::OK, &value)
}

/// Why registering a file stopped.
#[derive(Debug)]
pub enum DataRefusal {
    NotFound,
    Unavailable,
    Column(String),
    Parse(String),
}
impl DataRefusal {
    fn response(&self) -> Response {
        match self {
            Self::NotFound => http::not_found(),
            Self::Unavailable => http::unavailable(),
            Self::Column(column) => http::problem_with(
                StatusCode::UNPROCESSABLE_ENTITY,
                "data_refused",
                json!({"column": column}),
            ),
            Self::Parse(_) => http::problem(StatusCode::UNPROCESSABLE_ENTITY, "data_refused"),
        }
    }
}

/// Registers one data file of a box the member owns: `columns` as confirmed,
/// or the inferred defaults (`None`). Returns the registration and the table.
pub async fn register_file(
    state: &Boxes,
    member: &TrustedRequest,
    file: &BoxFile,
    confirmation: Option<&Value>,
) -> Result<(FileData, Table), DataRefusal> {
    let stores = state.auth.stores.as_ref().ok_or(DataRefusal::NotFound)?;
    let (bytes, table, inferred) = inspect_file(state, member, file)
        .await
        .map_err(DataRefusal::Parse)?;
    let columns = match confirmation {
        Some(body) => {
            inspect::confirmed(&inferred, body).ok_or(DataRefusal::Parse("columns".into()))?
        }
        None => inferred,
    };
    let normalized = register::normalize_and_store(
        &state.normalizer,
        &state.store,
        member.organization(),
        &file.name,
        &file.kind,
        &bytes,
        &table,
        &columns,
    )
    .await
    .map_err(|error| match error {
        RegisterError::Column(column) => DataRefusal::Column(column),
        RegisterError::Refused(code) => DataRefusal::Parse(code),
        RegisterError::Unavailable => DataRefusal::Unavailable,
    })?;
    // APP-113 (storage 6): a repeat of the same normalized content returns
    // the stored registration, so the answer names what storage holds.
    let stored = stores
        .workspace
        .register_box_data(
            member,
            &file.box_id,
            &file.file_id,
            &storage::NewRegistration {
                source_id: &normalized.source_id,
                revision_id: &normalized.revision_id,
                content_digest: &file.content_digest,
                normalized_digest: &normalized.normalized_digest,
                contract_digest: &normalized.contract_digest,
                envelope: &normalized.envelope,
                row_count: normalized.row_count,
            },
        )
        .await
        .map_err(|error| match error {
            Error::Denied => DataRefusal::NotFound,
            _ => DataRefusal::Unavailable,
        })?;
    tracing::info!(
        event = "box.data_registered",
        organization_id = member.organization(),
        box_id = file.box_id,
        file_id = file.file_id,
        source_id = stored.source_id,
        revision_id = stored.revision_id,
        rows = stored.row_count,
        "box data registered"
    );
    Ok((stored, table))
}

/// A saved work placed from a box (data card or derived app).
#[derive(Clone, Debug)]
pub struct Placed {
    pub app: String,
    pub work: String,
    pub title: String,
}

/// The composer context with the given revisions certainly among its sources.
async fn context_with(
    state: &Boxes,
    member: &TrustedRequest,
    data: &[&FileData],
) -> Result<storage::ComposerContext, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    let mut context = stores.workspace.composer_context(member).await?;
    for d in data {
        if !context
            .sources
            .iter()
            .any(|s| s.source == d.source_id && s.revision == d.revision_id)
        {
            context.sources.push(derive::readable(d));
        }
    }
    Ok(context)
}

/// A rule-built definition published on the member's authoring app.
async fn publish_rule_built(
    state: &Boxes,
    member: &TrustedRequest,
    context: &storage::ComposerContext,
    content: &Value,
    intent: &str,
) -> Result<Placed, Refused> {
    let stores = state.auth.stores.as_ref().ok_or(Refused::NotFound)?;
    let authoring = context
        .authoring
        .as_ref()
        .ok_or(Refused::CouldNotCompose("no_authoring_app"))?;
    let admitted = composer::plan::admit(content, member.organization(), &authoring.app)
        .map_err(|r| Refused::CouldNotCompose(r.code))?;
    composer::plan::require_runnable(content).map_err(|r| Refused::CouldNotCompose(r.code))?;
    let planned = composer::plan::Planned {
        title: content["title"].as_str().unwrap_or("Data").to_owned(),
        admitted,
        origin: composer::Origin::Generated,
    };
    let composed =
        composer::routes::publish_planned(stores, member, context, planned, intent).await?;
    Ok(Placed {
        app: composed.app,
        work: composed.work,
        title: composed.title,
    })
}

/// A file's data card (its totals and rows), recorded as `data` provenance.
pub async fn data_card(
    state: &Boxes,
    member: &TrustedRequest,
    file: &BoxFile,
    data: &FileData,
) -> Option<Placed> {
    let stores = state.auth.stores.as_ref()?;
    let context = context_with(state, member, &[data]).await.ok()?;
    let content = derive::data_definition(&file.name, data.row_count, &data.contract);
    let placed = publish_rule_built(
        state,
        member,
        &context,
        &content,
        &format!("Data card: {}", file.name),
    )
    .await
    .ok()?;
    stores
        .workspace
        .record_box_derivation(
            member,
            &file.box_id,
            &placed.app,
            &placed.work,
            "data",
            &[(file.file_id.clone(), file.content_digest.clone())],
        )
        .await
        .ok()?;
    Some(placed)
}

fn placed_json(placed: &Option<Placed>) -> Value {
    placed.as_ref().map_or(
        Value::Null,
        |p| json!({"app_id": p.app, "work_id": p.work, "title": p.title}),
    )
}

async fn owned_file(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
    file_id: &str,
) -> Result<BoxFile, Response> {
    let stores = state.auth.stores.as_ref().ok_or_else(http::not_found)?;
    let view = stores
        .workspace
        .read_box(member, box_id)
        .await
        .map_err(refusal)?;
    if !view.row.owner {
        return Err(http::not_found());
    }
    view.files
        .into_iter()
        .find(|f| f.file_id == file_id)
        .ok_or_else(http::not_found)
}

async fn data(
    State(state): State<Arc<Boxes>>,
    UniformPath((box_id, file_id)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) || !valid(&file_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    let object = match json_of(body, &parts.headers).await {
        Ok(object) => object,
        Err(response) => return response,
    };
    let card = match object.get("card") {
        None => false,
        Some(Value::Bool(card)) => *card,
        Some(_) => return http::problem(StatusCode::BAD_REQUEST, "malformed_request"),
    };
    if !object.keys().all(|k| k == "columns" || k == "card") {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_request");
    }
    let file = match owned_file(&state, &member, &box_id, &file_id).await {
        Ok(file) if matches!(file.kind.as_str(), "csv" | "xlsx") => file,
        Ok(_) => return http::not_found(),
        Err(response) => return response,
    };
    let (registered, _) = match register_file(&state, &member, &file, object.get("columns")).await {
        Ok(done) => done,
        Err(refused) => return refused.response(),
    };
    let placed = if card {
        data_card(&state, &member, &file, &registered).await
    } else {
        None
    };
    document(
        StatusCode::CREATED,
        &json!({"data": data_json(&registered), "card": placed_json(&placed)}),
    )
}

async fn attach(
    State(state): State<Arc<Boxes>>,
    UniformPath((box_id, file_id)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) || !valid(&file_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    let value = match json_of(body, &parts.headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (Some(app), Some(work)) = (
        value
            .get("app_id")
            .and_then(Value::as_str)
            .filter(|v| valid(v)),
        value
            .get("work_id")
            .and_then(Value::as_str)
            .filter(|v| valid(v)),
    ) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_request");
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    let file = match owned_file(&state, &member, &box_id, &file_id).await {
        Ok(file) => file,
        Err(response) => return response,
    };
    // The member must open the work; its app box is theirs.
    let outline = match stores.data.work_outline(&member, app, work).await {
        Ok(outline) => outline,
        Err(error) => return refusal(error),
    };
    let target = match stores
        .workspace
        .create_box(
            &member,
            &outline.title.chars().take(60).collect::<String>(),
            "app",
            Some(app),
        )
        .await
    {
        Ok(row) => row,
        Err(error) => return refusal(error),
    };
    let copied = if target.box_id == box_id {
        Some(file.clone())
    } else {
        let Some(new_id) = crate::auth::capability::random_uuid() else {
            return http::unavailable();
        };
        let key = store::file_key(member.organization(), &target.box_id, &new_id);
        let Ok(bytes) = state
            .store
            .get(
                member.organization(),
                &file.object_key,
                &file.content_digest,
            )
            .await
        else {
            return http::unavailable();
        };
        if state
            .store
            .put(member.organization(), &key, &bytes)
            .await
            .is_err()
        {
            return http::unavailable();
        }
        match stores
            .workspace
            .add_box_file(
                &member,
                &target.box_id,
                &storage::NewFile {
                    file_id: &new_id,
                    name: &file.name,
                    kind: &file.kind,
                    media_type: &file.media_type,
                    bytes: file.bytes,
                    content_digest: &file.content_digest,
                    object_key: &key,
                },
            )
            .await
        {
            Ok(Some(file)) => Some(file),
            // The row was definitely not written: no object stays behind.
            Ok(None) => {
                state.store.discard(member.organization(), &key).await;
                None
            }
            Err(error) => {
                if error == storage::Error::Denied {
                    state.store.discard(member.organization(), &key).await;
                }
                return refusal(error);
            }
        }
    };
    let Some(copied) = copied else {
        return http::problem(StatusCode::CONFLICT, "box_full");
    };
    let card = if matches!(copied.kind.as_str(), "csv" | "xlsx") {
        match register_file(&state, &member, &copied, None).await {
            Ok((registered, _)) => data_card(&state, &member, &copied, &registered).await,
            Err(_) => None,
        }
    } else {
        None
    };
    let number = number_of(&state, &member, &target.box_id).await;
    document(
        StatusCode::OK,
        &json!({"box": box_json(&target, number), "file": file_json(&copied), "card": placed_json(&card)}),
    )
}

/// An app built from a box, with per-step timings.
#[derive(Clone, Debug)]
pub struct Built {
    pub placed: Placed,
    pub origin: &'static str,
    pub source: &'static str,
    pub box_id: String,
    pub box_name: String,
    pub files: usize,
    pub timings: Vec<(&'static str, u64)>,
    /// APP-124 **Make a card**: an app with exactly one card.
    pub one_card: bool,
    /// APP-124: the `/name` a one-card app was given (none: unnamed).
    pub address: Option<String>,
}

/// Why building from a box stopped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildRefusal {
    NotFound,
    Unavailable,
    ComposerUnavailable,
    /// No CSV/XLSX in the box could be used.
    NoData,
    CouldNotCompose(&'static str),
    /// A per-member window is full (`Retry-After` seconds).
    TooMany(u64),
}
impl BuildRefusal {
    pub fn response(&self) -> Response {
        match self {
            Self::TooMany(retry_after) => http::too_many_requests(*retry_after),
            Self::NotFound => http::not_found(),
            Self::Unavailable => http::unavailable(),
            Self::ComposerUnavailable => {
                http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable")
            }
            Self::NoData => http::problem(StatusCode::UNPROCESSABLE_ENTITY, "no_data_files"),
            Self::CouldNotCompose(_) => {
                http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_compose")
            }
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Unavailable => "unavailable",
            Self::ComposerUnavailable => "composer_unavailable",
            Self::NoData => "no_data_files",
            Self::CouldNotCompose(code) => code,
            Self::TooMany(_) => "too_many_requests",
        }
    }
}

/// The composer's per-member window (`COMPOSE_MEMBER`, shared with the
/// canvas composer), on every surface that builds from a folder.
pub fn admit_compose(state: &Boxes, member: &TrustedRequest) -> Result<(), BuildRefusal> {
    let key = format!("compose:{}:{}", member.organization(), member.principal());
    state
        .auth
        .limits
        .admit(&[(&key, composer::routes::COMPOSE_MEMBER)])
        .map_err(BuildRefusal::TooMany)
}
/// The explanation and brief window (`EXPLAIN_MEMBER`), on every surface.
pub fn admit_explain(state: &Boxes, member: &TrustedRequest) -> Result<(), BuildRefusal> {
    let key = format!(
        "box-explain:{}:{}",
        member.organization(),
        member.principal()
    );
    state
        .auth
        .limits
        .admit(&[(&key, EXPLAIN_MEMBER)])
        .map_err(BuildRefusal::TooMany)
}
fn ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

/// Builds an app from a box the member owns (ticket APP-108 §5): its data
/// files registered (inferred defaults for any not yet confirmed), the capped
/// context, the model's typed definition (or, in a demo organization, the
/// rehearsed metrics definition, DS-73) admitted and published on the
/// member's authoring app, saved, and its provenance recorded.
pub async fn build_from_box(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
    purpose: &str,
    only: Option<&[String]>,
) -> Result<Built, BuildRefusal> {
    let started = Instant::now();
    let mut timings = Vec::new();
    if state.auth.stores.is_none() {
        return Err(BuildRefusal::NotFound);
    }
    if !state.composer.available_for(member.organization())
        && !crate::demo::is_demo_org(member.organization())
    {
        return Err(BuildRefusal::ComposerUnavailable);
    }
    let step = Instant::now();
    let (view, data) = box_data(state, member, box_id, only, super::MAX_CONTEXT_DATA_FILES).await?;
    let data = derive::distinct(data);
    timings.push(("parse_ms", ms(step)));
    if data.is_empty() {
        return Err(BuildRefusal::NoData);
    }
    build_with(state, member, box_id, purpose, view, data, started, timings).await
}

/// The folder (its owner only, else the uniform 404) and up to `at_most`
/// of its data files, registered (inferred defaults for any not yet
/// confirmed), each with its first rows: confirmed files first (the member
/// chose their types), then the newest others. A file that cannot be read
/// or registered is skipped and the next one tried, so a bad newest file
/// never hides an older good table (APP-124 review).
async fn box_data(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
    only: Option<&[String]>,
    at_most: usize,
) -> Result<(storage::BoxView, Vec<DataFile>), BuildRefusal> {
    let stores = state.auth.stores.as_ref().ok_or(BuildRefusal::NotFound)?;
    let view = stores
        .workspace
        .read_box(member, box_id)
        .await
        .map_err(|e| match e {
            Error::Denied => BuildRefusal::NotFound,
            _ => BuildRefusal::Unavailable,
        })?;
    if !view.row.owner {
        return Err(BuildRefusal::NotFound);
    }
    let mut data: Vec<DataFile> = Vec::new();
    let mut candidates: Vec<&BoxFile> = view
        .files
        .iter()
        .filter(|f| matches!(f.kind.as_str(), "csv" | "xlsx"))
        .filter(|f| only.is_none_or(|ids| ids.contains(&f.file_id)))
        .collect();
    candidates.sort_by_key(|f| (f.data.is_none(), std::cmp::Reverse(f.created_at.clone())));
    for file in candidates {
        if data.len() >= at_most {
            break;
        }
        let (registered, table) = match &file.data {
            Some(registered) => match inspect_file(state, member, file).await {
                Ok((_, table, _)) => (registered.clone(), table),
                Err(_) => continue,
            },
            None => match register_file(state, member, file, None).await {
                Ok(done) => done,
                Err(_) => continue,
            },
        };
        data.push(DataFile {
            file: file.clone(),
            data: registered,
            sample: table
                .rows
                .iter()
                .take(super::MAX_SAMPLE_ROWS)
                .cloned()
                .collect(),
        });
    }
    Ok((view, data))
}

/// The model (or rehearsed) half of [`build_from_box`].
#[allow(clippy::too_many_arguments)]
async fn build_with(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
    purpose: &str,
    view: storage::BoxView,
    data: Vec<DataFile>,
    started: Instant,
    mut timings: Vec<(&'static str, u64)>,
) -> Result<Built, BuildRefusal> {
    let stores = state.auth.stores.as_ref().ok_or(BuildRefusal::NotFound)?;
    let step = Instant::now();
    let refs: Vec<&FileData> = data.iter().map(|d| &d.data).collect();
    let context = context_with(state, member, &refs)
        .await
        .map_err(|_| BuildRefusal::Unavailable)?;
    let capped = derive::context(&view.row.name, &view.files, &data);
    let sources: Vec<storage::ReadableSource> = refs.iter().map(|d| derive::readable(d)).collect();
    timings.push(("context_ms", ms(step)));
    let intent = format!(
        "{} (from the {} {})",
        purpose.chars().take(400).collect::<String>(),
        super::NOUN_LOWER,
        view.row.name
    );
    let demo = crate::demo::is_demo_org(member.organization());
    let (placed, source) = if demo && derive::rehearsed(purpose) {
        let step = Instant::now();
        let content = derive::metrics_definition(&view.row.name, purpose, &data);
        let placed = publish_rule_built(state, member, &context, &content, &intent)
            .await
            .map_err(build_refusal)?;
        timings.push(("generation_ms", 0));
        timings.push(("admission_publish_ms", ms(step)));
        (placed, "rehearsed")
    } else {
        let authoring = context
            .authoring
            .as_ref()
            .ok_or(BuildRefusal::CouldNotCompose("no_authoring_app"))?;
        if !state.composer.enabled() {
            return Err(BuildRefusal::ComposerUnavailable);
        }
        let step = Instant::now();
        let context_bytes = capped.to_string().len();
        let deadline = Instant::now() + composer::COMPOSE_DEADLINE - Duration::from_secs(10);
        let generated = state
            .composer
            .generate_with(
                purpose,
                Some(&capped),
                &sources,
                member.organization(),
                &authoring.app,
                deadline,
            )
            .await
            .map_err(|failure| match failure {
                composer::GenerationFailure::Model(composer::model::ModelError::Unavailable) => {
                    BuildRefusal::ComposerUnavailable
                }
                composer::GenerationFailure::Model(error) => {
                    BuildRefusal::CouldNotCompose(error.code())
                }
                composer::GenerationFailure::Refused(_) => {
                    BuildRefusal::CouldNotCompose("admission_refused")
                }
                composer::GenerationFailure::OutOfTime => {
                    BuildRefusal::CouldNotCompose("out_of_time")
                }
            })?;
        timings.push(("generation_ms", ms(step)));
        tracing::info!(
            event = "box.context_sent",
            organization_id = member.organization(),
            box_id,
            context_bytes,
            data_files = data.len(),
            files = view.files.len().min(super::MAX_CONTEXT_FILES),
            attempts = generated.refused.len() + 1,
            "box context sent to the model"
        );
        let step = Instant::now();
        let composed =
            composer::routes::publish_planned(stores, member, &context, generated.planned, &intent)
                .await
                .map_err(build_refusal)?;
        timings.push(("admission_publish_ms", ms(step)));
        (
            Placed {
                app: composed.app,
                work: composed.work,
                title: composed.title,
            },
            "model",
        )
    };
    let used: Vec<(String, String)> = data
        .iter()
        .map(|d| (d.file.file_id.clone(), d.file.content_digest.clone()))
        .collect();
    stores
        .workspace
        .record_box_derivation(member, box_id, &placed.app, &placed.work, "derived", &used)
        .await
        .map_err(|_| BuildRefusal::Unavailable)?;
    timings.push(("total_ms", ms(started)));
    tracing::info!(
        event = "box.app_derived",
        organization_id = member.organization(),
        box_id,
        app_id = placed.app,
        work_id = placed.work,
        source,
        files = used.len(),
        total_ms = ms(started),
        "app built from box"
    );
    Ok(Built {
        placed,
        origin: "generated",
        source,
        box_id: box_id.to_owned(),
        box_name: view.row.name,
        files: used.len(),
        timings,
        one_card: false,
        address: None,
    })
}
/// APP-124 **Make a card**: formation constrained to one card. A minimal app
/// named after the folder (or, for Uploads or an untitled folder, the file)
/// with exactly one card, the most useful view of the folder's first data
/// file (`derive::card_definition`, by rule: no model call, so it works in
/// every organization), published on the member's authoring app, linked
/// back to the folder (`derived` provenance, the "Built from folder" chip)
/// and saved under a `/name` when one is free (`derive::card_slug`, then
/// `-card`, then `-2` … `-9`; never a verb, a reserved word, a library
/// address or alias, a name the member already sees or one of their
/// folders' addresses, nor one an organization label holds). The name is
/// judged whole under the organization's naming lock, as `POST
/// /v1/work/{app}/{work}/name` judges it, before anything is published.
pub async fn card_from_box(
    state: &Boxes,
    member: &TrustedRequest,
    box_id: &str,
    only: Option<&[String]>,
) -> Result<Built, BuildRefusal> {
    let started = Instant::now();
    let mut timings = Vec::new();
    let stores = state.auth.stores.as_ref().ok_or(BuildRefusal::NotFound)?;
    let step = Instant::now();
    let (view, data) = box_data(state, member, box_id, only, 1).await?;
    timings.push(("parse_ms", ms(step)));
    let Some(first) = data.into_iter().next() else {
        return Err(BuildRefusal::NoData);
    };
    let step = Instant::now();
    let context = context_with(state, member, &[&first.data])
        .await
        .map_err(|_| BuildRefusal::Unavailable)?;
    let title = derive::card_title(&view.row.name, &view.row.kind, &first.file.name);
    let content = derive::card_definition(&title, &first);
    let intent = format!(
        "One card from the {} {}: {}",
        super::NOUN_LOWER,
        view.row.name,
        first.file.name
    );
    let lock = composer::commands::naming_lock(member.organization());
    let naming = lock.lock().await;
    // The name is judged whole before anything is published (organization
    // labels included), so a held name never leaves an orphan publication.
    let address = free_address(state, member, &derive::card_slug(&title)).await;
    let saved_as = match &address {
        Some(slug) => format!("{}{slug}\n{intent}", storage::COMMAND_MARKER),
        None => intent.clone(),
    };
    let placed = match publish_rule_built(state, member, &context, &content, &saved_as).await {
        Ok(placed) => placed,
        // Only a label written by another task between the judgment above
        // and the save refuses the name now (storage judges labels under its
        // own advisory lock): the publication is logged as orphaned
        // (`composer.publication_orphaned`) and nothing is republished; a
        // retry judges again and takes the next free name.
        Err(Refused::NotFound) if address.is_some() => return Err(BuildRefusal::Unavailable),
        Err(refused) => return Err(build_refusal(refused)),
    };
    drop(naming);
    timings.push(("generation_ms", 0));
    timings.push(("admission_publish_ms", ms(step)));
    let used = [(
        first.file.file_id.clone(),
        first.file.content_digest.clone(),
    )];
    stores
        .workspace
        .record_box_derivation(member, box_id, &placed.app, &placed.work, "derived", &used)
        .await
        .map_err(|_| BuildRefusal::Unavailable)?;
    timings.push(("total_ms", ms(started)));
    tracing::info!(
        event = "box.card_derived",
        organization_id = member.organization(),
        box_id,
        app_id = placed.app,
        work_id = placed.work,
        named = address.is_some(),
        total_ms = ms(started),
        "one-card app built from box"
    );
    Ok(Built {
        placed,
        origin: "generated",
        source: "rule",
        box_id: box_id.to_owned(),
        box_name: view.row.name,
        files: 1,
        timings,
        one_card: true,
        address,
    })
}

/// The first free `/name` for a one-card app (see [`card_from_box`]), or
/// none. The caller holds the organization's naming lock. A folder's name is
/// compared as the canvas addresses it ([`derive::address_slug`], the
/// canvas's `folderSlug`); an organization label's address or kept address
/// is judged in storage, organization-wide, before anything is published.
async fn free_address(state: &Boxes, member: &TrustedRequest, base: &str) -> Option<String> {
    let stores = state.auth.stores.as_ref()?;
    let named = stores.data.named_works(member).await.ok()?;
    let folders: Vec<String> = stores
        .workspace
        .list_boxes(member)
        .await
        .ok()?
        .iter()
        .map(|b| derive::address_slug(folder_label(b)))
        .collect();
    let library = composer::matcher::library_slugs();
    let taken = |slug: &str| {
        composer::commands::RESERVED.contains(&slug)
            || composer::labels::VERBS.contains(&slug)
            || library.contains(slug)
            || named.iter().any(|n| n.slug == slug)
            || folders.iter().any(|f| f == slug)
    };
    let candidates: Vec<String> = card_addresses(base)
        .into_iter()
        .filter(|slug| composer::commands::valid_slug(slug) && !taken(slug))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let held = stores
        .workspace
        .labels_holding(member, &candidates)
        .await
        .ok()?;
    candidates.into_iter().find(|slug| !held.contains(slug))
}

/// The names a one-card app is offered, in order: the base, `-card`, then
/// `-2` … `-9`.
pub fn card_addresses(base: &str) -> Vec<String> {
    std::iter::once(base.to_owned())
        .chain(std::iter::once(format!("{base}-card")))
        .chain((2..=9).map(|n| format!("{base}-{n}")))
        .collect()
}

/// A folder's label as the canvas shows it (`boxes.ts` `boxLabel`).
fn folder_label(row: &BoxRow) -> &str {
    if row.kind == "uploads" {
        "Uploads"
    } else {
        &row.name
    }
}

fn build_refusal(refused: Refused) -> BuildRefusal {
    match refused {
        Refused::NotFound => BuildRefusal::NotFound,
        Refused::StoreUnavailable => BuildRefusal::Unavailable,
        Refused::ComposerUnavailable => BuildRefusal::ComposerUnavailable,
        Refused::CouldNotCompose(code) => BuildRefusal::CouldNotCompose(code),
    }
}

pub fn built_json(built: &Built) -> Value {
    let mut timings = serde_json::Map::new();
    for (name, value) in &built.timings {
        timings.insert((*name).into(), json!(value));
    }
    let mut value = json!({"app_id": built.placed.app, "work_id": built.placed.work, "title": built.placed.title,
        "origin": built.origin, "source": built.source, "box_id": built.box_id, "box_name": built.box_name,
        "files": built.files, "label": format!("Built from {}: {} · {} {}", super::NOUN_LOWER, built.box_name, built.files, if built.files == 1 { "file" } else { "files" }),
        "timings": timings});
    if let (true, Some(object)) = (built.one_card, value.as_object_mut()) {
        object.insert("cards".into(), json!(1));
        object.insert("address".into(), json!(built.address));
    }
    value
}

async fn apps(
    State(state): State<Arc<Boxes>>,
    UniformPath(box_id): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(refused) = admit_compose(&state, &member) {
        return refused.response();
    }
    let value = match json_of(body, &parts.headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let purpose = value
        .get("purpose")
        .and_then(Value::as_str)
        .and_then(composer::prompt::intent)
        .map(str::to_owned);
    let only: Option<Vec<String>> = match value.get("file_ids") {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => match items
            .iter()
            .map(|v| v.as_str().filter(|s| valid(s)).map(str::to_owned))
            .collect::<Option<Vec<_>>>()
        {
            Some(ids) if (1..=20).contains(&ids.len()) => Some(ids),
            _ => return http::problem(StatusCode::BAD_REQUEST, "malformed_request"),
        },
        Some(_) => return http::problem(StatusCode::BAD_REQUEST, "malformed_request"),
    };
    // APP-124: `"one_card": true` is **Make a card** (formation constrained
    // to one card, no purpose needed); anything but a boolean is refused.
    let one_card = match value.get("one_card") {
        None => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_intent"),
    };
    let closed = value
        .keys()
        .all(|k| k == "purpose" || k == "file_ids" || k == "one_card");
    let card = one_card && closed && (value.get("purpose").is_none() || purpose.is_some());
    let purpose = match (card, purpose, closed, one_card) {
        (true, _, _, _) => None,
        (false, Some(purpose), true, false) => Some(purpose),
        _ => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_intent"),
    };
    // ADR-011 I4 (APP-124 review): an `Idempotency-Key` makes Make a card
    // and folder formation idempotent: a repeat answers the first result.
    let key = match parts.headers.get("idempotency-key") {
        None => None,
        Some(header) => match header
            .to_str()
            .ok()
            .filter(|k| storage::idempotency_key_shaped(k))
        {
            Some(key) => Some(key.to_owned()),
            None => return http::problem(StatusCode::BAD_REQUEST, "malformed_request"),
        },
    };
    let run = async {
        let built = match &purpose {
            None => card_from_box(&state, &member, &box_id, only.as_deref()).await,
            Some(purpose) => {
                build_from_box(&state, &member, &box_id, purpose, only.as_deref()).await
            }
        };
        built.map(|built| built_json(&built))
    };
    let Some(key) = key else {
        return match run.await {
            Ok(built) => document(StatusCode::CREATED, &built),
            Err(refused) => refused.response(),
        };
    };
    let scope = format!(
        "{}:{}:{box_id}:{key}",
        member.organization(),
        member.principal()
    );
    let Some(first) = replay_claim(scope, Value::Object(value.clone()).to_string()) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused");
    };
    // A repeat waits for the first request with the key, then answers its
    // result; a refused or abandoned first request leaves nothing, so the
    // repeat runs.
    let mut first = first.lock().await;
    if let Some(built) = first.as_ref() {
        tracing::info!(
            event = "box.build_replayed",
            organization_id = member.organization(),
            box_id,
            app_id = built["app_id"].as_str().unwrap_or(""),
            "folder build answered from its idempotency key"
        );
        let mut response = document(StatusCode::CREATED, built);
        response
            .headers_mut()
            .insert("idempotent-replayed", HeaderValue::from_static("true"));
        return response;
    }
    match run.await {
        Ok(built) => {
            let response = document(StatusCode::CREATED, &built);
            *first = Some(built);
            response
        }
        Err(refused) => refused.response(),
    }
}

/// How long a folder build's idempotency key answers its first result, and
/// how many keys the process keeps (the oldest are forgotten first).
const REPLAY_FOR: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_REPLAYS: usize = 4096;

/// One idempotency key of `POST /v1/boxes/{box}/apps`: the body it was
/// first sent with and, once that request succeeded, its answer.
struct Replay {
    body: String,
    at: Instant,
    first: Arc<tokio::sync::Mutex<Option<Value>>>,
}

/// ADR-011 I4 for Make a card and folder formation (APP-124 review): the
/// first result of a `(organization, member, folder, key)`, kept in this
/// process for [`REPLAY_FOR`], as the naming lock is (the pitch runs one
/// task; a storage table is the multi-task fix, recorded in APP-124).
/// `None` when the key was first sent with another body.
pub(super) fn replay_claim(
    scope: String,
    body: String,
) -> Option<Arc<tokio::sync::Mutex<Option<Value>>>> {
    static REPLAYS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Replay>>,
    > = std::sync::OnceLock::new();
    let mut replays = REPLAYS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();
    replays.retain(|_, replay| now.duration_since(replay.at) < REPLAY_FOR);
    if let Some(replay) = replays.get(&scope) {
        return (replay.body == body).then(|| replay.first.clone());
    }
    while replays.len() >= MAX_REPLAYS {
        let oldest = replays
            .iter()
            .min_by_key(|(_, replay)| replay.at)
            .map(|(scope, _)| scope.clone());
        match oldest {
            Some(oldest) => replays.remove(&oldest),
            None => break,
        };
    }
    let first = Arc::new(tokio::sync::Mutex::new(None));
    replays.insert(
        scope,
        Replay {
            body,
            at: now,
            first: first.clone(),
        },
    );
    Some(first)
}

async fn rename(
    State(state): State<Arc<Boxes>>,
    UniformPath(box_id): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry) = admit_write(&state, &member) {
        return http::too_many_requests(retry);
    }
    let value = match json_of(body, &parts.headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(name) = value
        .get("name")
        .and_then(Value::as_str)
        .and_then(storage::box_name)
    else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_request");
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    match stores.workspace.rename_box(&member, &box_id, &name).await {
        Ok(row) => {
            let number = number_of(&state, &member, &row.box_id).await;
            document(StatusCode::OK, &json!({"box": box_json(&row, number)}))
        }
        Err(error) => refusal(error),
    }
}

async fn share(
    State(state): State<Arc<Boxes>>,
    UniformPath(box_id): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&box_id) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry) = admit_write(&state, &member) {
        return http::too_many_requests(retry);
    }
    let value = match json_of(body, &parts.headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(grantee) = value
        .get("grantee_principal_id")
        .and_then(Value::as_str)
        .filter(|v| valid(v))
    else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_request");
    };
    let Some(stores) = &state.auth.stores else {
        return http::not_found();
    };
    match stores.workspace.share_box(&member, &box_id, grantee).await {
        Ok(_) => document(
            StatusCode::OK,
            &json!({"box_id": box_id, "grantee_principal_id": grantee, "shared": true}),
        ),
        Err(error) => refusal(error),
    }
}

pub async fn lineage_json(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Option<Value>, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    Ok(stores
        .workspace
        .box_lineage(member, app, work)
        .await?
        .map(|lineage| {
            json!({"box_id": lineage.box_id, "box_name": lineage.box_name, "kind": lineage.kind,
            "file_count": lineage.file_count, "noun": super::NOUN,
            "files": lineage.files.iter().map(|(file, data)| json!({
                "file_id": file.file_id, "name": file.name, "kind": file.kind,
                "source_id": data.as_ref().map(|d| d.source_id.clone()),
                "revision_id": data.as_ref().map(|d| d.revision_id.clone()),
                "content_url": format!("/v1/boxes/{}/files/{}/content", file.box_id, file.file_id),
            })).collect::<Vec<_>>()})
        }))
}

async fn lineage(
    State(state): State<Arc<Boxes>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !valid(&app) || !valid(&work) {
        return http::not_found();
    }
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    match lineage_json(&state, &member, &app, &work).await {
        Ok(Some(value)) => document(StatusCode::OK, &value),
        Ok(None) => http::not_found(),
        Err(error) => refusal(error),
    }
}

/// Runs a work's data views (at most 8) under the member's authority; each
/// released result with its receipt digest.
pub async fn run_views(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Vec<explain::ViewResult>, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    let outline = stores.data.work_outline(member, app, work).await?;
    let mut results = Vec::new();
    for view in outline
        .views
        .iter()
        .filter(|v| !matches!(v.kind.as_str(), "form" | "note"))
        .take(8)
    {
        let document = match crate::query::routes::run_released(
            &state.work,
            member,
            app,
            work,
            &view.id,
            &json!({}),
            std::time::Instant::now(),
        )
        .await
        {
            Ok(document) => document,
            Err(crate::query::routes::ViewRefusal::Storage(Error::Denied)) => {
                return Err(Error::Denied)
            }
            Err(_) => continue,
        };
        results.push(explain::ViewResult {
            view_id: view.id.clone(),
            title: view.title.clone(),
            columns: document["columns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(str::to_owned))
                .collect(),
            column_types: document["column_types"].clone(),
            rows: document["rows"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r.as_array().cloned())
                .collect(),
            receipt_digest: document["receipt"]["digest"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        });
    }
    Ok(results)
}

/// The explanation of a derived work, filled for this member; `state: none`
/// when there is none yet or a reference no longer resolves.
pub async fn explanation(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Value, Error> {
    let stores = state.auth.stores.as_ref().ok_or(Error::Denied)?;
    if stores
        .workspace
        .box_lineage(member, app, work)
        .await?
        .is_none_or(|l| l.kind != "derived")
    {
        return Err(Error::Denied);
    }
    let none =
        json!({"state": "none", "label": explain::LABEL, "generated_at": null, "findings": []});
    let Some((template, at)) = stores
        .workspace
        .read_box_explanation(member, app, work)
        .await?
    else {
        return Ok(none);
    };
    let Some(findings) = crate::admission::decode(&template)
        .ok()
        .as_ref()
        .and_then(explain::read_template)
    else {
        return Ok(none);
    };
    let results = run_views(state, member, app, work).await?;
    Ok(match explain::fill(&findings, &results) {
        Some(filled) => {
            json!({"state": "ready", "label": explain::LABEL, "generated_at": at, "findings": filled})
        }
        None => none,
    })
}

/// Generates (or regenerates) the explanation: the views run, the fast model
/// (or the demo's rehearsed rule, DS-74) answers, the answer is judged, the
/// template stored, and the filled explanation returned with timings.
pub async fn explain_now(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Value, BuildRefusal> {
    let started = Instant::now();
    let stores = state.auth.stores.as_ref().ok_or(BuildRefusal::NotFound)?;
    let lineage = stores
        .workspace
        .box_lineage(member, app, work)
        .await
        .map_err(|e| match e {
            Error::Denied => BuildRefusal::NotFound,
            _ => BuildRefusal::Unavailable,
        })?;
    if lineage.is_none_or(|l| l.kind != "derived") {
        return Err(BuildRefusal::NotFound);
    }
    let step = Instant::now();
    let results = run_views(state, member, app, work)
        .await
        .map_err(|_| BuildRefusal::Unavailable)?;
    let views_ms = ms(step);
    let step = Instant::now();
    let demo = crate::demo::is_demo_org(member.organization());
    let (findings, source, input_bytes) = if demo {
        (explain::rehearsed(&results), "rehearsed", 0)
    } else {
        if !state.composer.enabled() {
            return Err(BuildRefusal::ComposerUnavailable);
        }
        let input = explain::model_input(&results);
        let bytes = input.len();
        match state
            .composer
            .complete_fast(explain::SYSTEM, input, 800, EXPLAIN_BUDGET)
            .await
        {
            Ok(reply) => (explain::judge(&reply.text, &results), "model", bytes),
            Err(composer::model::ModelError::Unavailable) => {
                return Err(BuildRefusal::ComposerUnavailable)
            }
            Err(_) => (None, "model", bytes),
        }
    };
    let summary_ms = ms(step);
    tracing::info!(
        event = "box.explained",
        organization_id = member.organization(),
        app_id = app,
        work_id = work,
        source,
        input_bytes,
        accepted = findings.is_some(),
        views = results.len(),
        views_ms,
        summary_ms,
        "explanation judged"
    );
    let timings = json!({"views_ms": views_ms, "summary_ms": summary_ms, "total_ms": ms(started)});
    let Some(findings) = findings else {
        return Ok(
            json!({"state": "none", "label": explain::LABEL, "generated_at": null, "findings": [], "timings": timings}),
        );
    };
    let template = crate::admission::encode(&explain::template(&findings, source))
        .map_err(|_| BuildRefusal::Unavailable)?;
    let at = stores
        .workspace
        .save_box_explanation(member, app, work, &template)
        .await
        // APP-113 (storage 8): saving a summary needs record_write; a reader
        // without it is the uniform 404, never a 503.
        .map_err(|error| match error {
            Error::Denied => BuildRefusal::NotFound,
            _ => BuildRefusal::Unavailable,
        })?;
    let filled = explain::fill(&findings, &results).ok_or(BuildRefusal::Unavailable)?;
    Ok(
        json!({"state": "ready", "label": explain::LABEL, "generated_at": at, "findings": filled, "source": source, "timings": timings}),
    )
}

async fn explain_get(
    State(state): State<Arc<Boxes>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !valid(&app) || !valid(&work) {
        return http::not_found();
    }
    let Some(member) = session(&state, request.headers()).await else {
        return http::not_found();
    };
    match explanation(&state, &member, &app, &work).await {
        Ok(value) => document(StatusCode::OK, &value),
        Err(error) => refusal(error),
    }
}

async fn explain_post(
    State(state): State<Arc<Boxes>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, _) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    if !valid(&app) || !valid(&work) {
        return http::not_found();
    }
    let Some(member) = session(&state, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(refused) = admit_explain(&state, &member) {
        return refused.response();
    }
    match explain_now(&state, &member, &app, &work).await {
        Ok(value) => document(StatusCode::OK, &value),
        Err(refused) => refused.response(),
    }
}
