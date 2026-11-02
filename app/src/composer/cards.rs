//! APP-123: cards made by you and pinned answers (`storage::cards`,
//! migration 0015).
//!
//! - `GET /v1/cards`: `{"cards": [...]}`, the member's own made cards and
//!   pinned answers and the cards added for everyone on apps they can open.
//! - `POST /v1/cards`: stores one, named by the client's `card_id` (a
//!   repeat of the same card is `{"saved": false}`):
//!   - a made card: `{"card_id", "kind": "card", "app_id", "work_id",
//!     "card", "address", "title", "collection" | "view", "spec"}`. The spec
//!     is judged against the card's fields as the server reads them from the
//!     work's published schema (`talk::schema_fields`; never the client's
//!     field list): unknown fields, values, keys or `"receipts": false` are
//!     422 with the judge's code (`unknown_field`, `drops_receipts` …); a
//!     records card may carry an answer's record set (`records`, "Make it a
//!     card"). The read that authorizes it is Shape's grants-only one
//!     (`Data::read_only_schema`);
//!   - a pinned answer: `{"card_id", "kind": "answer", "question", "text",
//!     "scope": {"kind", "address"}, "references", "receipts"}` exactly as
//!     Ask answered them; every receipt is checked against storage (a
//!     revision of a record of its work and a publication of its app,
//!     `Data::receipts_known`, under `record_read`): one storage never issued
//!     is 422 `unknown_receipt`. It never runs again.
//!     Optional `app_id`/`work_id` retain the owning app even without records.
//!     A file answer additionally supplies `file_source: {box_id,file_id}` and
//!     `file_references` (row references), with empty record references/receipts.
//!     Fresh lineage supplies the canonical file identity and original digest;
//!     the write and later listings recheck exact file and app access.
//! - `GET /v1/cards` lists a card only while the member holds `record_read`
//!   on its work and on every work a pinned answer's receipts cite, and a
//!   card added for everyone only while its maker is a member.
//! - `POST /v1/cards/{card}/everyone` `{"everyone": true|false}`: the maker
//!   adds their card for every member who can read its app's work (the
//!   app's owner or an admin only), or keeps it for themselves; the app's
//!   owner or an admin may take another member's card added for everyone
//!   back to its maker (`false` only, `can_manage`); audited `card.shared`.
//! - `POST /v1/cards/{card}/remove` `{}`: the maker removes it (a soft
//!   delete; for everyone when it was added for everyone), or the app's
//!   owner or an admin removes a card added for everyone; audited
//!   `card.removed`.
//!
//! Refusals: the uniform 404 for no session, a card, app or work of another
//! organization, one the member cannot open or did not make (unless they
//! steward a card added for everyone), and (sharing) a member who is
//! neither the app's owner nor an admin; 403 `cross_site`;
//! 413; 422 `invalid_request`, a judge's code, `unknown_receipt`,
//! `card_exists`, `too_many_cards`, `not_shareable`; 429; 503. Logs carry
//! identities, kinds and codes, never a question, an answer or a spec.
use super::{ask, shape, talk};
use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    storage::{self, Carded, Error, MemberCard, NewCard, ReceiptKey},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use serde_json::{json, Map, Value};
use std::sync::Arc;

pub const CARDS_PATH: &str = "/v1/cards";
pub const EVERYONE_PATH: &str = "/v1/cards/{card}/everyone";
pub const REMOVE_PATH: &str = "/v1/cards/{card}/remove";
/// A pinned answer may cite up to [`ask::MAX_REFERENCES`] records, each with its receipt.
const BODY_LIMIT: usize = 393_216;
const TITLE_CHARS: usize = 120;
const LABEL_CHARS: usize = 200;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(CARDS_PATH, get(list).post(make))
        .route(EVERYONE_PATH, post(everyone))
        .route(REMOVE_PATH, post(remove))
        .with_state(auth)
}

/// The product supplies the parser/object store belonging to this instance;
/// record-only consumers can keep using `router` without a file dependency.
pub fn router_with_boxes(boxes: Arc<crate::boxes::routes::Boxes>) -> Router {
    router(boxes.auth.clone()).layer(Extension(boxes))
}

fn document(value: &Value) -> Response {
    match crate::admission::encode(value) {
        Ok(bytes) => {
            let mut response = (StatusCode::OK, bytes).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            http::secure(response.headers_mut());
            response
        }
        Err(_) => http::unavailable(),
    }
}

fn invalid() -> Response {
    http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request")
}

/// A card as the canvas reads it.
pub fn card_json(card: &MemberCard) -> Value {
    json!({
        "card_id": card.card,
        "kind": card.kind,
        "app_id": card.app,
        "work_id": card.work,
        "source": card.source,
        "address": card.address,
        "title": card.title,
        "spec": card.spec,
        "everyone": card.everyone,
        "mine": card.mine,
        "can_share": card.can_share,
        "can_manage": card.can_manage,
        "created_at": card.created_at,
    })
}

fn answer(result: Result<Carded<MemberCard>, Error>) -> Response {
    match result {
        Ok(Carded::Saved(card)) => document(&json!({"saved": true, "card": card_json(&card)})),
        Ok(Carded::Unchanged(card)) => document(&json!({"saved": false, "card": card_json(&card)})),
        Ok(Carded::Refused(code)) => http::problem(StatusCode::UNPROCESSABLE_ENTITY, code),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// A title: trimmed, 1..=120 characters, no control characters.
fn title(value: &str) -> Option<String> {
    let value = value.trim();
    ((1..=TITLE_CHARS).contains(&value.chars().count()) && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

/// The card a made card came from (`records`, `by-status`, `shape-1` …).
fn source_card(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// A made card's body, its spec not yet judged.
#[derive(Debug)]
pub(crate) struct CardBody {
    pub card: String,
    pub app: String,
    pub work: String,
    pub source: String,
    pub address: String,
    pub title: String,
    pub from: talk::CardSource,
    pub spec: Map<String, Value>,
}

pub(crate) fn card_body(object: &Map<String, Value>) -> Option<CardBody> {
    if !talk::exactly(
        object,
        &[
            "card_id", "kind", "app_id", "work_id", "card", "address", "title", "spec",
        ],
        &["collection", "view"],
    ) {
        return None;
    }
    let id = |key: &str| {
        object
            .get(key)?
            .as_str()
            .filter(|v| storage::valid_identifier(v))
            .map(str::to_owned)
    };
    let from = match (object.get("collection"), object.get("view")) {
        (Some(collection), None) => {
            let collection = collection.as_str()?;
            storage::valid_identifier(collection)
                .then(|| talk::CardSource::Collection(collection.to_owned()))?
        }
        (None, Some(view)) => {
            let view = view.as_str()?;
            talk::view_id(view).then(|| talk::CardSource::View(view.to_owned()))?
        }
        _ => return None,
    };
    let source = object.get("card")?.as_str()?;
    Some(CardBody {
        card: id("card_id")?,
        app: id("app_id")?,
        work: id("work_id")?,
        source: source_card(source).then(|| source.to_owned())?,
        address: talk::address(object.get("address"))?,
        title: title(object.get("title")?.as_str()?)?,
        from,
        spec: object.get("spec")?.as_object()?.clone(),
    })
}

/// A pinned answer's body: the closed answer, its receipts as storage keys.
#[derive(Debug)]
pub(crate) struct AnswerBody {
    pub card: String,
    pub title: String,
    pub address: String,
    /// The stored spec: question, text, scope, references, receipts.
    pub spec: Value,
    pub receipts: Vec<ReceiptKey>,
    /// An explicit app owner survives answers that read no records.
    pub owner: Option<(String, String)>,
    pub file: Option<FileAnswer>,
}

#[derive(Debug)]
pub(crate) struct FileAnswer {
    box_id: String,
    file_id: String,
    references: Vec<Value>,
}

fn timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            19 => *b == b'Z',
            _ => b.is_ascii_digit(),
        })
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn reference_name(value: &str) -> bool {
    value
        .strip_prefix('r')
        .is_some_and(|n| (1..=5).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

pub(crate) fn answer_body(object: &Map<String, Value>) -> Option<AnswerBody> {
    if !talk::exactly(
        object,
        &[
            "card_id",
            "kind",
            "question",
            "text",
            "scope",
            "references",
            "receipts",
        ],
        &["app_id", "work_id", "file_source", "file_references"],
    ) {
        return None;
    }
    let card = object
        .get("card_id")?
        .as_str()
        .filter(|v| storage::valid_identifier(v))?
        .to_owned();
    let question = ask::question(object.get("question")?.as_str()?)?.to_owned();
    let text = object.get("text")?.as_str()?.trim().to_owned();
    if !(1..=ask::MAX_TEXT).contains(&text.chars().count())
        || text.chars().any(|c| c.is_control() && c != '\n')
    {
        return None;
    }
    let scope = object.get("scope")?.as_object()?;
    if !talk::exactly(scope, &["kind"], &["address"])
        || !matches!(scope.get("kind")?.as_str()?, "card" | "app" | "canvas")
    {
        return None;
    }
    let address = talk::address(scope.get("address"))?;
    let owner = match (object.get("app_id"), object.get("work_id")) {
        (None, None) => None,
        (Some(app), Some(work)) => {
            let (app, work) = (app.as_str()?, work.as_str()?);
            if !storage::valid_identifier(app)
                || !storage::valid_identifier(work)
                || scope.get("kind")?.as_str()? == "canvas"
            {
                return None;
            }
            Some((app.to_owned(), work.to_owned()))
        }
        _ => return None,
    };
    let references = object.get("references")?.as_array()?;
    let receipts = object.get("receipts")?.as_array()?;
    if references.len() > ask::MAX_REFERENCES || receipts.len() != references.len() {
        return None;
    }
    let mut keys: Vec<ReceiptKey> = Vec::with_capacity(receipts.len());
    let mut names: Vec<&str> = Vec::with_capacity(references.len());
    for (reference, receipt) in references.iter().zip(receipts) {
        let reference = reference.as_object()?;
        let receipt = receipt.as_object()?;
        if !talk::exactly(
            reference,
            &["ref", "app_id", "work_id", "record_id", "address", "label"],
            &[],
        ) || !talk::exactly(
            receipt,
            &[
                "kind",
                "reference",
                "app_id",
                "work_id",
                "collection_id",
                "record_id",
                "version",
                "commit_sequence",
                "committed_at",
                "publication_id",
                "definition_digest",
                "work_version",
            ],
            &[],
        ) {
            return None;
        }
        let string = |object: &Map<String, Value>, key: &str| -> Option<String> {
            object.get(key)?.as_str().map(str::to_owned)
        };
        let ids_ok = ["app_id", "work_id", "record_id"].iter().all(|key| {
            string(reference, key).is_some_and(|v| storage::valid_identifier(&v))
                && string(reference, key) == string(receipt, key)
        });
        let name = reference.get("ref")?.as_str()?;
        let label = reference.get("label")?.as_str()?;
        let positive = |key: &str| receipt.get(key)?.as_i64().filter(|n| *n >= 1);
        if !ids_ok
            || !reference_name(name)
            || receipt.get("reference")?.as_str()? != name
            || names.contains(&name)
            || receipt.get("kind")?.as_str()? != "record"
            || label.chars().count() > LABEL_CHARS
            || label.chars().any(char::is_control)
            || talk::address(reference.get("address")).is_none()
            || !timestamp(receipt.get("committed_at")?.as_str()?)
            || !digest(receipt.get("definition_digest")?.as_str()?)
            || receipt.get("work_version")?.as_i64().is_none_or(|n| n < 0)
        {
            return None;
        }
        names.push(name);
        keys.push(ReceiptKey {
            app: string(receipt, "app_id")?,
            work: string(receipt, "work_id")?,
            collection: string(receipt, "collection_id")
                .filter(|v| storage::valid_identifier(v))?,
            record: string(receipt, "record_id")?,
            version: positive("version")?,
            commit_sequence: positive("commit_sequence")?,
            publication: string(receipt, "publication_id")?,
            definition_digest: string(receipt, "definition_digest")?,
        });
    }
    // An app-owned answer may not smuggle another app's records under its
    // parent. Global canvas answers retain their existing multi-work form.
    if owner
        .as_ref()
        .is_some_and(|(app, work)| keys.iter().any(|key| &key.app != app || &key.work != work))
    {
        return None;
    }
    let file = match (object.get("file_source"), object.get("file_references")) {
        (None, None) => None,
        (Some(source), Some(references)) => {
            if owner.is_none() || !keys.is_empty() || scope.get("kind")?.as_str()? != "app" {
                return None;
            }
            let source = source.as_object()?;
            if !talk::exactly(source, &["box_id", "file_id"], &[]) {
                return None;
            }
            let box_id = source.get("box_id")?.as_str()?;
            let file_id = source.get("file_id")?.as_str()?;
            if !storage::valid_identifier(box_id) || !storage::valid_identifier(file_id) {
                return None;
            }
            let references = references.as_array()?;
            if references.len() > crate::boxes::ask::MAX_ROWS {
                return None;
            }
            let mut rows = Vec::new();
            for reference in references {
                let reference = reference.as_object()?;
                if !talk::exactly(
                    reference,
                    &["ref", "file_id", "file", "sha256", "sheet", "row", "label"],
                    &[],
                ) {
                    return None;
                }
                let row = reference.get("row")?.as_u64()?;
                let name = reference.get("file")?.as_str()?;
                let label = reference.get("label")?.as_str()?;
                let sheet = reference.get("sheet")?;
                if row == 0
                    || row > crate::boxes::ask::MAX_ROWS as u64
                    || rows.contains(&row)
                    || reference.get("ref")?.as_str()? != format!("f1r{row}")
                    || reference.get("file_id")?.as_str()? != file_id
                    || !digest(reference.get("sha256")?.as_str()?)
                    || !(1..=200).contains(&name.chars().count())
                    || name.chars().any(char::is_control)
                    || label.chars().count() > LABEL_CHARS
                    || label.chars().any(char::is_control)
                    || !(sheet.is_null()
                        || sheet.as_str().is_some_and(|s| {
                            !s.is_empty()
                                && s.chars().count() <= 8192
                                && !s.chars().any(char::is_control)
                        }))
                {
                    return None;
                }
                rows.push(row);
            }
            Some(FileAnswer {
                box_id: box_id.to_owned(),
                file_id: file_id.to_owned(),
                references: references.clone(),
            })
        }
        _ => return None,
    };
    let title: String = question.chars().take(TITLE_CHARS).collect();
    Some(AnswerBody {
        card,
        title: title.trim().to_owned(),
        spec: json!({
            "question": question,
            "text": text,
            "scope": {"kind": scope.get("kind")?, "address": address},
            "references": references,
            "receipts": receipts,
        }),
        address,
        receipts: keys,
        owner,
        file,
    })
}

/// File provenance comes from a freshly authorized lineage, never a client URL
/// or asserted file name. Text/labels remain the saved answer, not a new model run.
fn file_answer_spec(body: &FileAnswer, file: &storage::BoxFile) -> Option<Value> {
    if body.box_id != file.box_id || body.file_id != file.file_id {
        return None;
    }
    for reference in &body.references {
        if reference["file"] != file.name
            || reference["sha256"] != file.content_digest
            || (file.kind == "csv" && !reference["sheet"].is_null())
            || !matches!(file.kind.as_str(), "csv" | "xlsx")
            || file.data.as_ref().is_some_and(|data| {
                reference["row"]
                    .as_i64()
                    .is_none_or(|row| row > data.row_count)
            })
        {
            return None;
        }
    }
    Some(
        json!({"box_id": file.box_id, "file_id": file.file_id, "name": file.name,
        "kind": file.kind, "sha256": file.content_digest}),
    )
}

fn file_answer_rows_match(
    body: &FileAnswer,
    file: &storage::BoxFile,
    table: &crate::boxes::inspect::Table,
) -> bool {
    body.references.iter().all(|reference| {
        reference["row"]
            .as_u64()
            .and_then(|row| usize::try_from(row).ok())
            .and_then(|row| crate::boxes::ask::original_reference(file, table, row))
            .is_some_and(|original| &original == reference)
    })
}

/// The member of a session-gated write: same-origin, a live session, the
/// member write limit and the bounded body.
async fn front(
    auth: &Auth,
    request: Request,
) -> Result<(storage::TrustedRequest, axum::body::Bytes), Response> {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return Err(http::cross_site());
    }
    if parts.uri.query().is_some() {
        return Err(http::not_found());
    }
    let Some((_, member)) = crate::sharing::session(auth, &parts.headers).await else {
        return Err(http::not_found());
    };
    if let Err(retry_after) = auth
        .limits
        .admit_member_write(member.organization(), member.principal())
    {
        return Err(http::too_many_requests(retry_after));
    }
    let Some(bytes) = http::bounded(body, BODY_LIMIT).await else {
        return Err(http::payload_too_large());
    };
    Ok((member, bytes))
}

fn log(member: &storage::TrustedRequest, event: &'static str, response: &Response) {
    tracing::info!(
        event,
        status = response.status().as_u16(),
        organization_id = member.organization(),
        principal_id = member.principal(),
        "member card"
    );
}

/// `GET /v1/cards`.
async fn list(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.member_cards(&member).await {
        Ok(cards) => document(&json!({"cards": cards.iter().map(card_json).collect::<Vec<_>>()})),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// `POST /v1/cards`.
async fn make(
    State(auth): State<Arc<Auth>>,
    boxes: Option<Extension<Arc<crate::boxes::routes::Boxes>>>,
    request: Request,
) -> Response {
    let (member, bytes) = match front(&auth, request).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Ok(value) = crate::admission::decode(&bytes) else {
        return invalid();
    };
    let Some(object) = value.as_object() else {
        return invalid();
    };
    let card = match object.get("kind").and_then(Value::as_str) {
        Some("card") => {
            let Some(body) = card_body(object) else {
                return invalid();
            };
            let (collections, outline) = match stores
                .data
                .read_only_schema(&member, &body.app, &body.work)
                .await
            {
                Ok(schema) => schema,
                Err(Error::Denied) => return http::not_found(),
                Err(_) => return http::unavailable(),
            };
            let Some(fields) = talk::schema_fields(&body.from, &collections, &outline) else {
                return invalid();
            };
            let records = matches!(body.from, talk::CardSource::Collection(_));
            let view = match shape::judge_stored_card(body.spec, &fields, records) {
                Ok(view) => view,
                Err(code) => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, code),
            };
            let from = match &body.from {
                talk::CardSource::Collection(collection) => json!({"collection": collection}),
                talk::CardSource::View(view) => json!({"view": view}),
            };
            let Ok(view) = serde_json::to_value(&view) else {
                return http::unavailable();
            };
            let Ok(spec) = crate::admission::encode(&json!({"from": from, "view": view})) else {
                return invalid();
            };
            NewCard {
                card: body.card,
                kind: "card",
                app: Some(body.app),
                work: Some(body.work),
                source: body.source,
                address: body.address,
                title: body.title,
                spec,
                reads: Vec::new(),
            }
        }
        Some("answer") => {
            let Some(mut body) = answer_body(object) else {
                return invalid();
            };
            let mut works: Vec<(&str, &str)> = Vec::new();
            if let Some((app, work)) = &body.owner {
                works.push((app.as_str(), work.as_str()));
            }
            for key in &body.receipts {
                if !works.contains(&(key.app.as_str(), key.work.as_str())) {
                    works.push((key.app.as_str(), key.work.as_str()));
                }
            }
            if works.len() > storage::READ_ONLY_WORKS {
                return invalid();
            }
            if !works.is_empty() {
                match stores
                    .data
                    .receipts_known(&member, &works, &body.receipts)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unknown_receipt")
                    }
                    Err(Error::Denied) => return http::not_found(),
                    Err(_) => return http::unavailable(),
                }
            }
            if let Some(source) = &body.file {
                let Some((app, work)) = &body.owner else {
                    return invalid();
                };
                let lineage = match stores.workspace.box_lineage(&member, app, work).await {
                    Ok(Some(lineage)) if lineage.box_id == source.box_id => lineage,
                    Ok(_) | Err(Error::Denied) => return http::not_found(),
                    Err(_) => return http::unavailable(),
                };
                let Some((file, _)) = lineage
                    .files
                    .iter()
                    .find(|(file, _)| file.file_id == source.file_id)
                else {
                    return http::not_found();
                };
                let Some(canonical) = file_answer_spec(source, file) else {
                    return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unknown_receipt");
                };
                if !source.references.is_empty() {
                    // Ask and this pin use the same sealed original parser.
                    // A digest alone cannot certify a sheet name or row label.
                    let Some(Extension(state)) = &boxes else {
                        return http::unavailable();
                    };
                    let table = match crate::boxes::routes::inspect_file(state, &member, file).await
                    {
                        Ok((_, table, _)) => table,
                        Err(_) => {
                            return http::problem(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "source_unavailable",
                            )
                        }
                    };
                    if !file_answer_rows_match(source, file, &table) {
                        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "unknown_receipt");
                    }
                }
                body.spec["file_source"] = canonical;
                body.spec["file_references"] = json!(source.references);
            }
            let single = (works.len() == 1).then(|| works[0]);
            let Ok(spec) = crate::admission::encode(&body.spec) else {
                return invalid();
            };
            if spec.len() > storage::MAX_SPEC_BYTES {
                return http::payload_too_large();
            }
            NewCard {
                card: body.card,
                kind: "answer",
                app: single.map(|(app, _)| app.to_owned()),
                work: single.map(|(_, work)| work.to_owned()),
                source: String::new(),
                address: body.address,
                title: body.title,
                spec,
                reads: works
                    .iter()
                    .map(|(app, work)| ((*app).to_owned(), (*work).to_owned()))
                    .collect(),
            }
        }
        _ => return invalid(),
    };
    let response = answer(stores.workspace.make_card(&member, &card).await);
    log(
        &member,
        if card.kind == "card" {
            "card.made"
        } else {
            "answer.pinned"
        },
        &response,
    );
    response
}

/// `POST /v1/cards/{card}/everyone`.
async fn everyone(
    State(auth): State<Arc<Auth>>,
    UniformPath(card): UniformPath<String>,
    request: Request,
) -> Response {
    if !storage::valid_identifier(&card) {
        return http::not_found();
    }
    let (member, bytes) = match front(&auth, request).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let Some(everyone) = value
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("everyone"))
        .and_then(Value::as_bool)
    else {
        return invalid();
    };
    let response = answer(stores.workspace.share_card(&member, &card, everyone).await);
    log(&member, "card.shared", &response);
    response
}

/// `POST /v1/cards/{card}/remove`.
async fn remove(
    State(auth): State<Arc<Auth>>,
    UniformPath(card): UniformPath<String>,
    request: Request,
) -> Response {
    if !storage::valid_identifier(&card) {
        return http::not_found();
    }
    let (member, bytes) = match front(&auth, request).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if value.as_object().is_none_or(|object| !object.is_empty()) {
        return invalid();
    }
    let response = match stores.workspace.remove_card(&member, &card).await {
        Ok(Carded::Saved(card)) => document(&json!({"saved": true, "card_id": card})),
        Ok(Carded::Unchanged(card)) => document(&json!({"saved": false, "card_id": card})),
        Ok(Carded::Refused(code)) => http::problem(StatusCode::UNPROCESSABLE_ENTITY, code),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    };
    log(&member, "card.removed", &response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "00000000-0000-4000-8000-000000001681";
    const WORK: &str = "00000000-0000-4000-8000-000000001684";
    const COLLECTION: &str = "00000000-0000-4000-8000-000000001683";
    const CARD: &str = "00000000-0000-4000-8000-0000000c4a01";
    const RECORD: &str = "00000000-0000-4000-8000-00000000c001";
    const PUBLICATION: &str = "00000000-0000-4000-8000-0000000016a1";

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn made_card_bodies_are_closed() {
        let good = json!({"card_id": CARD, "kind": "card", "app_id": APP, "work_id": WORK, "card": "records",
            "address": "expenses.records", "title": "Red invoices", "collection": COLLECTION,
            "spec": {"filters": [{"field": "status", "op": "eq", "value": "submitted"}]}});
        let body = card_body(&object(good.clone())).unwrap();
        assert_eq!(
            body.from,
            talk::CardSource::Collection(COLLECTION.to_owned())
        );
        assert_eq!(body.title, "Red invoices");
        for (key, bad) in [
            ("fields", json!([{"id": "salary", "type": "money"}])),
            ("view", json!("by_status")),
            ("card_id", json!("1")),
            ("title", json!("")),
            ("title", json!("a\u{7}b")),
            ("address", json!("<b>")),
            ("card", json!("Records")),
            ("spec", json!("filters")),
        ] {
            let mut value = good.clone();
            value[key] = bad.clone();
            assert!(card_body(&object(value)).is_none(), "{key}: {bad}");
        }
        let mut no_source = good.clone();
        no_source.as_object_mut().unwrap().remove("collection");
        assert!(card_body(&object(no_source)).is_none());
    }

    fn receipt(reference: &str) -> Value {
        json!({"kind": "record", "reference": reference, "app_id": APP, "work_id": WORK, "collection_id": COLLECTION,
            "record_id": RECORD, "version": 1, "commit_sequence": 3, "committed_at": "2026-10-31T12:00:00Z",
            "publication_id": PUBLICATION, "definition_digest": format!("sha256:{}", "a".repeat(64)), "work_version": 4})
    }

    #[test]
    fn pinned_answers_are_closed_and_keep_their_receipts() {
        let good = json!({"card_id": CARD, "kind": "answer", "question": "? where are the red invoices",
            "text": "One needs attention.", "scope": {"kind": "card", "address": "expenses.ledger"},
            "references": [{"ref": "r1", "app_id": APP, "work_id": WORK, "record_id": RECORD, "address": "expenses", "label": "Nobu · $86.00"}],
            "receipts": [receipt("r1")]});
        let body = answer_body(&object(good.clone())).unwrap();
        assert_eq!(body.title, "where are the red invoices");
        assert_eq!(body.receipts.len(), 1);
        assert_eq!(body.receipts[0].commit_sequence, 3);
        assert_eq!(body.spec["receipts"][0], receipt("r1"));
        // A reference without its receipt, a receipt of another record, a
        // view spec, a forged digest or kind, an extra key: all refused.
        let mut dropped = good.clone();
        dropped["receipts"] = json!([]);
        let mut other = good.clone();
        other["receipts"][0]["record_id"] = json!(APP);
        let mut spec = good.clone();
        spec["highlight"] = json!([]);
        let mut forged = good.clone();
        forged["receipts"][0]["definition_digest"] = json!("sha256:xyz");
        let mut query = good.clone();
        query["receipts"][0]["kind"] = json!("query");
        let mut version = good.clone();
        version["receipts"][0]["version"] = json!(0);
        let mut renamed = good.clone();
        renamed["receipts"][0]["reference"] = json!("r2");
        for bad in [dropped, other, spec, forged, query, version, renamed] {
            assert!(answer_body(&object(bad.clone())).is_none(), "{bad}");
        }
        // An answer with no records pins too (no app).
        let mut plain = good.clone();
        plain["references"] = json!([]);
        plain["receipts"] = json!([]);
        assert!(answer_body(&object(plain)).unwrap().receipts.is_empty());
    }

    fn file_answer() -> Value {
        json!({"card_id": CARD, "kind": "answer", "app_id": APP, "work_id": WORK,
            "question": "Which row leads?", "text": "The first row.", "scope": {"kind": "app", "address": ""},
            "references": [], "receipts": [], "file_source": {"box_id": COLLECTION, "file_id": RECORD},
            "file_references": [{"ref": "f1r1", "file_id": RECORD, "file": "source.csv", "sha256": format!("sha256:{}", "a".repeat(64)), "sheet": null, "row": 1, "label": "A row"}]})
    }

    #[test]
    fn app_answer_ownership_is_explicit_and_cannot_mix_foreign_receipts() {
        let mut plain = file_answer();
        plain.as_object_mut().unwrap().remove("file_source");
        plain.as_object_mut().unwrap().remove("file_references");
        let body = answer_body(&object(plain.clone())).unwrap();
        assert_eq!(body.owner, Some((APP.into(), WORK.into())));
        assert!(body.receipts.is_empty());
        for key in ["app_id", "work_id"] {
            let mut missing = plain.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(answer_body(&object(missing)).is_none());
        }
        let mut global = plain.clone();
        global["scope"]["kind"] = json!("canvas");
        assert!(answer_body(&object(global)).is_none());
        plain["references"] = json!([{"ref": "r1", "app_id": APP, "work_id": WORK, "record_id": RECORD, "address": "", "label": "A row"}]);
        plain["receipts"] = json!([receipt("r1")]);
        assert!(answer_body(&object(plain.clone())).is_some());
        plain["work_id"] = json!(CARD);
        assert!(answer_body(&object(plain)).is_none());
    }

    #[test]
    fn file_answer_rows_are_closed_bounded_and_exactly_scoped() {
        let good = file_answer();
        assert_eq!(
            answer_body(&object(good.clone()))
                .unwrap()
                .file
                .unwrap()
                .references
                .len(),
            1
        );
        for (key, value) in [
            ("row", json!(0)),
            ("row", json!(201)),
            ("row", json!(1.5)),
            ("ref", json!("f2r1")),
            ("ref", json!("f1r2")),
            ("file_id", json!(APP)),
            ("sha256", json!("sha256:bogus")),
            ("label", json!("x".repeat(201))),
            ("label", json!("hidden\u{0007}control")),
            ("url", json!("https://untrusted.example")),
            ("sheet", json!("x".repeat(8193))),
            ("sheet", json!(4)),
        ] {
            let mut bad = good.clone();
            bad["file_references"][0][key] = value;
            assert!(answer_body(&object(bad)).is_none(), "{key}");
        }
        let mut duplicate = good.clone();
        duplicate["file_references"] =
            json!([good["file_references"][0], good["file_references"][0]]);
        assert!(answer_body(&object(duplicate)).is_none());
        let mut source = good.clone();
        source["file_source"]["sha256"] = json!("client-digest");
        assert!(answer_body(&object(source)).is_none());
        let mut owner = good.clone();
        owner.as_object_mut().unwrap().remove("app_id");
        owner.as_object_mut().unwrap().remove("work_id");
        assert!(answer_body(&object(owner)).is_none());
        let mut empty = good;
        empty["file_references"] = json!([]);
        assert!(
            answer_body(&object(empty)).is_some(),
            "a no-row answer still belongs to its original file"
        );
    }

    #[test]
    fn file_answer_provenance_uses_authorized_original_metadata() {
        let body = answer_body(&object(file_answer())).unwrap().file.unwrap();
        let file = storage::BoxFile {
            box_id: COLLECTION.into(),
            file_id: RECORD.into(),
            name: "source.csv".into(),
            kind: "csv".into(),
            media_type: "text/csv".into(),
            bytes: 42,
            content_digest: format!("sha256:{}", "a".repeat(64)),
            object_key: "private-original".into(),
            created_at: String::new(),
            data: None,
        };
        let spec = file_answer_spec(&body, &file).unwrap();
        assert_eq!(
            spec.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["box_id", "file_id", "kind", "name", "sha256"]
        );
        assert_eq!(spec["sha256"], file.content_digest);
        let mut wrong = file.clone();
        wrong.content_digest = format!("sha256:{}", "b".repeat(64));
        assert!(file_answer_spec(&body, &wrong).is_none());
        wrong = file.clone();
        wrong.box_id = APP.into();
        assert!(file_answer_spec(&body, &wrong).is_none());
        wrong = file.clone();
        wrong.name = "other.csv".into();
        assert!(file_answer_spec(&body, &wrong).is_none());
        wrong = file;
        wrong.data = Some(storage::FileData {
            source_id: APP.into(),
            revision_id: WORK.into(),
            normalized_digest: String::new(),
            row_count: 0,
            contract: json!({}),
            contract_digest: String::new(),
            created_at: String::new(),
        });
        assert!(
            file_answer_spec(&body, &wrong).is_none(),
            "a row beyond a registered table is refused"
        );
    }

    #[test]
    fn file_answer_citations_match_the_real_sheet_row_and_label_without_registration() {
        let mut answer = file_answer();
        answer["file_references"][0]["file"] = json!("source.xlsx");
        answer["file_references"][0]["sheet"] = json!("Actual sheet");
        let file = storage::BoxFile {
            box_id: COLLECTION.into(),
            file_id: RECORD.into(),
            name: "source.xlsx".into(),
            kind: "xlsx".into(),
            media_type: String::new(),
            bytes: 42,
            content_digest: format!("sha256:{}", "a".repeat(64)),
            object_key: String::new(),
            created_at: String::new(),
            data: None,
        };
        let table = crate::boxes::inspect::Table {
            format: "xlsx",
            sheet: Some("Actual sheet".into()),
            header: vec!["Name".into()],
            rows: vec![vec!["A row".into()]],
        };
        let body = answer_body(&object(answer.clone())).unwrap().file.unwrap();
        assert!(file_answer_rows_match(&body, &file, &table));
        for (key, value) in [
            ("sheet", json!("Invented sheet")),
            ("sheet", Value::Null),
            ("label", json!("Forged source value")),
        ] {
            let mut forged = answer.clone();
            forged["file_references"][0][key] = value;
            let body = answer_body(&object(forged)).unwrap().file.unwrap();
            assert!(!file_answer_rows_match(&body, &file, &table), "{key}");
        }
        answer["file_references"][0]["row"] = json!(2);
        answer["file_references"][0]["ref"] = json!("f1r2");
        let body = answer_body(&object(answer)).unwrap().file.unwrap();
        assert!(
            !file_answer_rows_match(&body, &file, &table),
            "a nonexistent row cannot be saved even for an unregistered source"
        );
    }

    #[test]
    fn stored_specs_are_judged_against_the_schema_fields() {
        let fields = shape::fields(&json!([
            {"id": "amount", "type": "money"},
            {"id": "status", "type": "enum", "values": ["submitted", "approved"]},
            {"id": "filed_by", "type": "text"},
        ]))
        .unwrap();
        let judge =
            |spec: Value, records: bool| shape::judge_stored_card(object(spec), &fields, records);
        assert!(judge(
            json!({"filters": [{"field": "status", "op": "eq", "value": "approved"}]}),
            true
        )
        .is_ok());
        assert_eq!(
            judge(json!({"columns": ["salary"]}), true),
            Err("unknown_field")
        );
        assert_eq!(
            judge(json!({"presentation": "number", "receipts": false}), true),
            Err("drops_receipts")
        );
        assert_eq!(judge(json!({"css": "x"}), true), Err("unknown_key"));
        // The record set of an answer: a records card only, ids only, at most 200.
        let set = json!({"records": [RECORD], "title": "Red invoices"});
        assert_eq!(judge(set.clone(), true).unwrap().records, [RECORD]);
        assert_eq!(judge(set, false), Err("unknown_key"));
        assert_eq!(
            judge(json!({"records": ["r1"]}), true),
            Err("invalid_records")
        );
        assert_eq!(
            judge(json!({"records": [RECORD, RECORD]}), true),
            Err("invalid_records")
        );
        // The model can never produce a record set.
        assert_eq!(
            shape::judge_card(&json!({"records": [RECORD]}).to_string(), &fields),
            Err("unknown_key")
        );
    }
}
