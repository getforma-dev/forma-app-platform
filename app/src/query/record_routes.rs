//! Record projections share one captured snapshot and one fresh release check.
//! The UI filter body is bounded data; it never selects another work or source.
use super::{
    record_components::{self, RecordComponentProps},
    routes,
    worker::INTERACTIVE_DEADLINE,
    Work,
};
use crate::{
    auth::csrf,
    http::{self, UniformPath},
    storage::{self, GuestAccess, TrustedRequest, WorkOutline},
};
use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Serialize)]
pub(crate) struct Batch {
    pub components: Vec<RecordComponentProps>,
    pub filter_epoch: i64,
    #[serde(skip)]
    pub run: Option<storage::RecordComponentsRun>,
}

pub(crate) fn is_record_kind(kind: &str) -> bool {
    matches!(
        kind,
        "records_list" | "record_summary" | "record_trend" | "record_ranking" | "record_budget"
    )
}

pub(crate) fn response(batch: &Batch) -> Response {
    match serde_json::to_vec(batch) {
        Ok(bytes) => {
            let mut response =
                ([(header::CONTENT_TYPE, "application/json")], bytes).into_response();
            http::secure(response.headers_mut());
            response
        }
        Err(_) => http::unavailable(),
    }
}

fn storage_refusal(error: storage::Error) -> Response {
    if error == storage::Error::ProjectionLimit {
        http::problem(StatusCode::UNPROCESSABLE_ENTITY, "record_projection_limit")
    } else {
        routes::storage_refusal(&error)
    }
}

pub(crate) async fn parameters(request: Request, origin: &str) -> Result<Value, Response> {
    if request.uri().query().is_some() {
        return Err(http::not_found());
    }
    if request.method() == Method::GET {
        return Ok(record_components::default_parameters());
    }
    if request.method() != Method::POST || !csrf::same_origin(request.headers(), origin) {
        return Err(http::cross_site());
    }
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err(http::problem(
            StatusCode::BAD_REQUEST,
            "invalid_record_filters",
        ));
    }
    let bytes = http::read_bounded(
        request.into_body(),
        http::JSON_BODY_LIMIT,
        http::HANDLER_DEADLINE,
    )
    .await
    .map_err(|_| http::payload_too_large())?;
    let value = crate::admission::decode(&bytes)
        .map_err(|_| http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_record_filters"))?;
    if !record_components::parameters_shaped(&value) {
        return Err(http::problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_record_filters",
        ));
    }
    Ok(value)
}

pub(crate) async fn member_components(
    work: &Work,
    member: &TrustedRequest,
    app: &str,
    saved: &str,
    parameters: &Value,
) -> Result<Batch, Response> {
    let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
    let run = stores
        .data
        .record_components(member, app, saved)
        .await
        .map_err(storage_refusal)?;
    let components = execute(work, &run, parameters).await;
    // Even an execution failure must not release names/fields after authority ends.
    stores
        .data
        .release_record_components(member, &run)
        .await
        .map_err(storage_refusal)?;
    Ok(Batch {
        components,
        filter_epoch: parameters["filter_epoch"].as_i64().unwrap_or(0),
        run: Some(run),
    })
}

pub(crate) async fn guest_components(
    work: &Work,
    access: &GuestAccess,
    parameters: &Value,
) -> Result<Batch, Response> {
    let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
    let run = stores
        .data
        .guest_record_components(access)
        .await
        .map_err(storage_refusal)?;
    let components = execute(work, &run, parameters).await;
    stores
        .data
        .release_guest_record_components(access, &run)
        .await
        .map_err(storage_refusal)?;
    Ok(Batch {
        components,
        filter_epoch: parameters["filter_epoch"].as_i64().unwrap_or(0),
        run: Some(run),
    })
}

async fn execute(
    work: &Work,
    run: &storage::RecordComponentsRun,
    parameters: &Value,
) -> Vec<RecordComponentProps> {
    if run.is_empty() {
        return Vec::new();
    }
    let Some(engine) = work.executor() else {
        return record_components::unavailable_with_parameters(
            run,
            parameters,
            "Record views are temporarily unavailable.",
        );
    };
    match engine
        .execute_records(run, parameters, INTERACTIVE_DEADLINE)
        .await
    {
        Ok(result) => record_components::props(run, &result, parameters).unwrap_or_else(|_| {
            record_components::unavailable_with_parameters(
                run,
                parameters,
                "The record result could not be verified. Try again.",
            )
        }),
        Err(_) => record_components::unavailable_with_parameters(
            run,
            parameters,
            "Record views could not be computed. Try again or narrow the filters.",
        ),
    }
}

/// Native documents keep forms usable on a read failure without showing zeros.
/// Their caller still performs its full final document authority/pin check.
pub(crate) fn unavailable_outline(outline: &WorkOutline) -> Vec<RecordComponentProps> {
    outline.parts.iter().filter(|part| is_record_kind(&part.kind)).map(|part| RecordComponentProps {
        id: part.id.clone(), title: part.title.clone(), kind: part.kind.clone(), state: "failed".into(), failed: true,
        message: "Record views are unavailable or exceed the current size limit. No totals are shown.".into(), ..Default::default()
    }).collect()
}

pub(crate) async fn member_route(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    if request.method() == Method::POST
        && !csrf::same_origin(request.headers(), work.auth().origin())
    {
        return http::cross_site();
    }
    let (_, member) = match routes::member(work.auth(), request.headers()).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let parameters = match parameters(request, work.auth().origin()).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match member_components(&work, &member, &app, &saved, &parameters).await {
        Ok(batch) => response(&batch),
        Err(response) => response,
    }
}
