//! Owner-reviewed, same-work editable copies of immutable uploaded data.
use super::routes::{self, Boxes};
use crate::{
    admission,
    auth::csrf,
    http::{self, UniformPath},
    storage::{self, DataCopyRows, FileData, PublishResult, TrustedRequest},
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde_json::{json, Value};
use std::{
    io::{Cursor, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
struct Staged(Option<PathBuf>);
impl Drop for Staged {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) fn acquire_import() -> Option<tokio::sync::OwnedSemaphorePermit> {
    static IMPORTS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    IMPORTS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
        .try_acquire_owned()
        .ok()
}
pub const PATH: &str = "/v1/work/{app}/{work}/data-copy";
pub fn long_path(path: &str) -> bool {
    let parts: Vec<_> = path.split('/').collect();
    matches!(parts.as_slice(), ["", "v1", "work", _, _, "data-copy"])
}
#[derive(Debug)]
pub enum Refusal {
    Storage(storage::Error),
    Limit,
    Refused,
    Stale,
    KeyReused,
    AlreadyEditable,
    Busy,
}
impl From<storage::Error> for Refusal {
    fn from(e: storage::Error) -> Self {
        Self::Storage(e)
    }
}
fn response(error: Refusal) -> Response {
    match error {
        Refusal::Busy => http::too_many_requests(2),
        Refusal::Storage(storage::Error::Denied) => http::not_found(),
        Refusal::Storage(_) => http::unavailable(),
        Refusal::Limit => http::problem(StatusCode::UNPROCESSABLE_ENTITY, "data_copy_limit"),
        Refusal::Refused => http::problem(StatusCode::UNPROCESSABLE_ENTITY, "data_copy_refused"),
        Refusal::Stale => http::problem(StatusCode::CONFLICT, "stale"),
        Refusal::KeyReused => {
            http::problem(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused")
        }
        Refusal::AlreadyEditable => http::problem(StatusCode::CONFLICT, "already_editable"),
    }
}

pub async fn materialize(
    state: &Boxes,
    member: &TrustedRequest,
    data: &[FileData],
) -> Result<Vec<DataCopyRows>, Refusal> {
    let total: i64 = data.iter().map(|f| f.row_count).sum();
    if total < 0 || total as usize > storage::DATA_COPY_ROWS {
        return Err(Refusal::Limit);
    }
    let mut copies = Vec::new();
    let mut total_bytes = 0usize;
    let mut transport_bytes = 2usize;
    for file in data {
        let mut path = state.normalizer.cache_root.join(format!(
            "o/{}/{}.{}.parquet",
            member.organization(),
            file.source_id,
            file.revision_id
        ));
        let mut staged = Staged(None);
        if !path.exists() {
            let key = super::store::revision_key(
                member.organization(),
                &file.source_id,
                &file.revision_id,
                "part.parquet",
            );
            let bytes = state
                .store
                .get(member.organization(), &key, &file.normalized_digest)
                .await
                .map_err(|_| Refusal::Refused)?;
            std::fs::create_dir_all(&state.normalizer.scratch_root)
                .map_err(|_| Refusal::Refused)?;
            path = state.normalizer.scratch_root.join(format!(
                "import-{}.parquet",
                crate::auth::capability::random_uuid().ok_or(Refusal::Refused)?
            ));
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(|_| Refusal::Refused)?;
            staged.0 = Some(path.clone());
            out.write_all(&bytes).map_err(|_| Refusal::Refused)?;
        }
        let request = json!({"path":path,"digest":file.normalized_digest,"fields":file.contract["fields"],"rows":file.row_count});
        let mut args = state.normalizer.args.clone();
        if let Some(last) = args.last_mut() {
            *last = "record-import".to_owned();
        } else {
            return Err(Refusal::Refused);
        }
        let mut command = tokio::process::Command::new(&state.normalizer.program);
        command.args(args);
        let output = crate::supervise::run_confined(
            command,
            Cursor::new(serde_json::to_vec(&request).map_err(|_| Refusal::Refused)?),
            &crate::supervise::Confinement {
                rss_limit_bytes: 1536 * 1024 * 1024,
                deadline: Duration::from_secs(60),
                nofile: 64,
                core: 0,
            },
        )
        .await
        .map_err(|_| Refusal::Refused)?;
        if !output.status.success() {
            return Err(Refusal::Refused);
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| Refusal::Refused)?;
        let rows = value["rows"].as_array().ok_or(Refusal::Refused)?.clone();
        if rows.len() != file.row_count as usize {
            return Err(Refusal::Refused);
        }
        let identity_overhead=serde_json::to_vec(&json!({"collection":"00000000-0000-4000-8000-000000000000","record":"00000000-0000-4000-8000-000000000000","source_input":file.contract["id"],"canonical":""})).map_err(|_|Refusal::Refused)?.len()+9;
        for row in &rows {
            let length = admission::encode(row).map_err(|_| Refusal::Refused)?.len();
            total_bytes += length;
            transport_bytes += length * 2 + identity_overhead;
        }
        if total_bytes > storage::DATA_COPY_BYTES
            || transport_bytes > storage::DATA_COPY_TRANSPORT_BYTES
        {
            return Err(Refusal::Limit);
        }
        copies.push(DataCopyRows {
            source_input: file.contract["id"]
                .as_str()
                .ok_or(Refusal::Refused)?
                .to_owned(),
            rows,
        });
    }
    Ok(copies)
}
async fn status(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Value, Refusal> {
    let stores = state
        .auth
        .stores
        .as_ref()
        .ok_or(Refusal::Storage(storage::Error::Denied))?;
    let premise = stores
        .workspace
        .work_presentation(member, app, work)
        .await?;
    let copied = stores.workspace.data_copy_rows(member, app, work).await?;
    let editable = premise.content["record_schemas"]
        .as_array()
        .is_some_and(|v| !v.is_empty());
    let lineage = stores.workspace.box_lineage(member, app, work).await;
    let files: Vec<Value> = match &lineage {
        Ok(Some(l)) => l
            .files
            .iter()
            .filter_map(|(file, data)| {
                data.as_ref()
                    .map(|d| json!({"name":file.name,"rows":d.row_count}))
            })
            .collect(),
        Ok(None) | Err(storage::Error::Denied) => Vec::new(),
        Err(e) => return Err((*e).into()),
    };
    let total: i64 = files.iter().filter_map(|f| f["rows"].as_i64()).sum();
    let mut can_enable = !editable
        && premise.can_edit
        && storage::editable_definition(&premise.content).is_ok()
        && !files.is_empty();
    let reason = if total as usize > storage::DATA_COPY_ROWS {
        can_enable = false;
        Some("data_copy_limit")
    } else if !can_enable && !editable {
        Some("data_copy_unavailable")
    } else {
        None
    };
    let mut value = json!({"app_id":app,"work_id":work,"editable":editable,"can_enable":can_enable,"publication_id":premise.publication,"work_version":premise.version,"total_rows":total,"files":files,"limits":{"rows":storage::DATA_COPY_ROWS,"bytes":storage::DATA_COPY_BYTES}});
    if let Some(reason) = reason {
        value["reason"] = json!(reason);
    }
    if let Some(rows) = copied {
        value["imported_rows"] = json!(rows);
    }
    Ok(value)
}
pub async fn enable(
    state: &Boxes,
    member: &TrustedRequest,
    app: &str,
    work: &str,
    publication: &str,
    version: i64,
    key: &str,
) -> Result<Value, Refusal> {
    let stores = state
        .auth
        .stores
        .as_ref()
        .ok_or(Refusal::Storage(storage::Error::Denied))?;
    let premise = stores
        .workspace
        .work_presentation(member, app, work)
        .await?;
    if !premise.can_edit {
        return Err(storage::Error::Denied.into());
    }
    if let Some(receipt) = stores
        .workspace
        .data_copy_receipt(member, app, work)
        .await?
    {
        if receipt.key != key {
            return Err(Refusal::AlreadyEditable);
        }
        if receipt.publication != publication || receipt.version != version {
            return Err(Refusal::KeyReused);
        }
        return status(state, member, app, work).await;
    }
    if premise.publication != publication || premise.version != version {
        return Err(Refusal::Stale);
    }
    let (content, plans) =
        storage::editable_definition(&premise.content).map_err(|_| Refusal::Refused)?;
    let lineage = stores
        .workspace
        .box_lineage(member, app, work)
        .await?
        .ok_or(Refusal::Refused)?;
    let data: Vec<FileData> = lineage
        .files
        .into_iter()
        .filter_map(|(_, d)| d)
        .filter(|d| plans.iter().any(|p| d.contract["id"] == p.source_input))
        .collect();
    let _capacity = acquire_import().ok_or(Refusal::Busy)?;
    let copies = materialize(state, member, &data).await?;
    let admitted = crate::composer::plan::admit(&content, member.organization(), app)
        .map_err(|_| Refusal::Refused)?;
    crate::composer::plan::require_runnable(&content).map_err(|_| Refusal::Refused)?;
    let revision = stores
        .workspace
        .create_definition_revision(member, app, &admitted)
        .await?;
    let publication = match stores
        .control
        .publish(member, app, &revision.revision, &premise.policy)
        .await?
    {
        PublishResult::Published(p) => p.publication_id,
        PublishResult::Stale(_) => return Err(Refusal::Stale),
    };
    if !stores
        .workspace
        .adopt_data_copy(member, app, work, &premise, &publication, key, &copies)
        .await?
    {
        return Err(Refusal::Stale);
    }
    status(state, member, app, work).await
}
pub async fn get(
    State(state): State<Arc<Boxes>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(member) = routes::session(&state, &headers).await else {
        return http::not_found();
    };
    match status(&state, &member, &app, &work).await {
        Ok(v) => routes::document(StatusCode::OK, &v),
        Err(e) => response(e),
    }
}
pub async fn post(
    State(state): State<Arc<Boxes>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !csrf::same_origin(request.headers(), state.auth.origin()) {
        return http::problem(StatusCode::FORBIDDEN, "cross_site");
    }
    let Some(member) = routes::session(&state, request.headers()).await else {
        return http::not_found();
    };
    let bytes = match axum::body::to_bytes(request.into_body(), 2048).await {
        Ok(b) => b,
        Err(_) => return http::problem(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let Ok(value) = admission::decode(&bytes) else {
        return http::problem(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Some(object) = value.as_object() else {
        return http::problem(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let args = (
        value["publication_id"].as_str(),
        value["expected_work_version"].as_i64(),
        value["idempotency_key"].as_str(),
    );
    let (Some(publication), Some(version), Some(key)) = args else {
        return http::problem(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if object.len() != 3 || !storage::idempotency_key_shaped(key) || version < 1 {
        return http::problem(StatusCode::BAD_REQUEST, "invalid_request");
    }
    match enable(&state, &member, &app, &work, publication, version, key).await {
        Ok(v) => routes::document(StatusCode::OK, &v),
        Err(e) => response(e),
    }
}
