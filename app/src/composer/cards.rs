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
//! - `POST /v1/cards/{card}/everyone` `{"everyone": true|false}`: the maker
//!   adds their card for every member who can open its app (the app's owner
//!   or an admin only), or keeps it for themselves; audited `card.shared`.
//! - `POST /v1/cards/{card}/remove` `{}`: the maker removes it (a soft
//!   delete; for everyone when it was added for everyone); audited
//!   `card.removed`.
//!
//! Refusals: the uniform 404 for no session, a card, app or work of another
//! organization, one the member cannot open or did not make, and (sharing)
//! a member who is neither the app's owner nor an admin; 403 `cross_site`;
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
    Router,
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
        &[],
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
async fn make(State(auth): State<Arc<Auth>>, request: Request) -> Response {
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
            }
        }
        Some("answer") => {
            let Some(body) = answer_body(object) else {
                return invalid();
            };
            let mut works: Vec<(&str, &str)> = Vec::new();
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
