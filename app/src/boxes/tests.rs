//! APP-108 route-level unit tests (no database): every route is mounted,
//! mutations refuse cross-site requests first, and without a session every
//! answer is the uniform 404.
use super::routes::{long_path, router, Boxes};
use crate::auth::Auth;
use axum::{body::Body, http::StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

const ID: &str = "00000000-0000-4000-8000-0000000000aa";

fn app() -> axum::Router {
    let auth = Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
    let work = Arc::new(crate::query::Work::new(
        auth.clone(),
        None,
        Arc::new(crate::query::routes::tests::RecordingPages::default()),
    ));
    let state = Arc::new(Boxes {
        auth,
        work,
        composer: crate::composer::Composer::new(None, "r", "g"),
        store: super::store::ObjectStore::Local {
            root: std::env::temp_dir().join("genui-box-routes-unused"),
        },
        normalizer: super::register::Normalizer::image(),
        parser: None,
    });
    crate::http::uniform(router(state))
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
