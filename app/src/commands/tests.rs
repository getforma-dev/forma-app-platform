//! Unit tests of the command route's pure parts (no Docker): the payload
//! digest reproduces the edition-1 contract vector, the JSON and form bodies
//! parse into the same request, form texts are typed by the echoed spec, the
//! notice set is closed, and the problem bodies carry their authorized members.
use super::{
    routes::{self, typed_value},
    Notice,
};
use crate::{
    http,
    storage::{CommandRequest, ConflictReason, RecoveryView},
};
use axum::{body::to_bytes, http::header};
use serde_json::{json, Value};

const FIXTURE: &str =
    include_str!("../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json");

fn fixture_request() -> (Value, Value) {
    let bundle: Value = serde_json::from_str(FIXTURE).unwrap();
    (bundle["command"].clone(), bundle["outcome"].clone())
}

#[test]
fn payload_digest_reproduces_the_contract_vector() {
    let (command, outcome) = fixture_request();
    let request = routes::parse_json_for_tests(
        &serde_json::to_vec(&command).unwrap(),
        command["app_id"].as_str().unwrap(),
        command["work_id"].as_str().unwrap(),
        command["command_id"].as_str().unwrap(),
        None,
    )
    .expect("the fixture command parses");
    assert!(request.well_formed());
    assert_eq!(
        super::payload::digest(&request).as_deref(),
        outcome["payload_digest"].as_str()
    );
    // The key is outside the identity: another key, same payload identity;
    // another value, another identity.
    let mut rekeyed = request.clone();
    rekeyed.idempotency_key = "ffffffffffffffffffffffffffffffff".into();
    assert_eq!(
        super::payload::digest(&rekeyed),
        super::payload::digest(&request)
    );
    let mut changed = request.clone();
    changed.values[1]["value"]["value"] = json!("Review required");
    assert_ne!(
        super::payload::digest(&changed),
        super::payload::digest(&request)
    );
    // A float is not forma-json-v1: no identity, and storage refuses it.
    let mut float = request.clone();
    float.values[0]["value"]["value"] = json!(1.5);
    assert_eq!(super::payload::digest(&float), None);
    // The request's document is the fixture command exactly.
    assert_eq!(request.document(), command);
}

#[test]
fn json_bodies_are_bound_to_the_path_and_the_header_key() {
    let (command, _) = fixture_request();
    let (app, work, id) = (
        command["app_id"].as_str().unwrap(),
        command["work_id"].as_str().unwrap(),
        command["command_id"].as_str().unwrap(),
    );
    let bytes = serde_json::to_vec(&command).unwrap();
    // The header key agrees with the body's; a disagreeing header refuses.
    assert!(routes::parse_json_for_tests(
        &bytes,
        app,
        work,
        id,
        Some("0123456789abcdef0123456789abcdef")
    )
    .is_some());
    assert!(routes::parse_json_for_tests(
        &bytes,
        app,
        work,
        id,
        Some("ffffffffffffffffffffffffffffffff")
    )
    .is_none());
    // A body naming another app, work or command than the path refuses.
    assert!(routes::parse_json_for_tests(&bytes, work, work, id, None).is_none());
    assert!(routes::parse_json_for_tests(&bytes, app, work, "other", None).is_none());
    // Unknown members, another kind and a missing key refuse.
    let mut extra = command.clone();
    extra["organization_id"] = json!(app);
    assert!(routes::parse_json_for_tests(
        &serde_json::to_vec(&extra).unwrap(),
        app,
        work,
        id,
        None
    )
    .is_none());
    let mut kind = command.clone();
    kind["kind"] = json!("command_outcome");
    assert!(
        routes::parse_json_for_tests(&serde_json::to_vec(&kind).unwrap(), app, work, id, None)
            .is_none()
    );
    let mut keyless = command.clone();
    keyless.as_object_mut().unwrap().remove("idempotency_key");
    assert!(routes::parse_json_for_tests(
        &serde_json::to_vec(&keyless).unwrap(),
        app,
        work,
        id,
        None
    )
    .is_none());
    assert!(routes::parse_json_for_tests(
        &serde_json::to_vec(&keyless).unwrap(),
        app,
        work,
        id,
        Some("0123456789abcdef0123456789abcdef")
    )
    .is_some());
    assert!(routes::parse_json_for_tests(b"not json", app, work, id, None).is_none());
}

#[test]
fn form_bodies_type_their_texts_by_the_echoed_spec_and_equal_the_json_request() {
    let (command, _) = fixture_request();
    let (app, work, id) = (
        command["app_id"].as_str().unwrap(),
        command["work_id"].as_str().unwrap(),
        command["command_id"].as_str().unwrap(),
    );
    let json =
        routes::parse_json_for_tests(&serde_json::to_vec(&command).unwrap(), app, work, id, None)
            .unwrap();
    let field = |name: &str, value: &str| (name.to_owned(), value.to_owned());
    let fields = vec![
        field("csrf", "token"),
        field("idempotency_key", "0123456789abcdef0123456789abcdef"),
        field("collection_id", "00000000-0000-4000-8000-0000000000ce"),
        field(
            "publication_id",
            command["publication_id"].as_str().unwrap(),
        ),
        field("schema_id", "decision"),
        field("schema_version_expected", "1"),
        field("record_id", command["record_id"].as_str().unwrap()),
        field("expected_record_version", "0"),
        field("expected_work_version", "1"),
        field("value.status", "needs_context"),
        field(
            "spec.status",
            "{\"type\":\"enum\",\"values\":[\"needs_context\",\"reviewed\"]}",
        ),
        field("value.note", "Review required — synthetic only"),
        field("spec.note", "{\"type\":\"text\"}"),
    ];
    let (form, collection) = routes::parse_form_for_tests(&fields, app, work, id).unwrap();
    assert_eq!(collection, "00000000-0000-4000-8000-0000000000ce");
    assert_eq!(form, json);
    // A value without its spec, a non-numeric version and a repeated premise
    // field refuse; an empty text is the null value.
    let mut specless = fields.clone();
    specless.retain(|(name, _)| name != "spec.note");
    assert!(routes::parse_form_for_tests(&specless, app, work, id).is_none());
    let mut version = fields.clone();
    version[8] = field("expected_work_version", "one");
    assert!(routes::parse_form_for_tests(&version, app, work, id).is_none());
    let mut repeated = fields.clone();
    repeated.push(field("record_id", command["record_id"].as_str().unwrap()));
    assert!(routes::parse_form_for_tests(&repeated, app, work, id).is_none());
    let mut empty = fields.clone();
    empty[11] = field("value.note", "");
    let (form, _) = routes::parse_form_for_tests(&empty, app, work, id).unwrap();
    assert_eq!(form.values[1]["value"], json!({"type": "null"}));
    // Every typed shape the contract admits, from its text.
    assert_eq!(
        typed_value(&json!({"type": "integer", "unit": "byte"}), "12"),
        Some(json!({"type": "integer", "value": "12", "unit": "byte"}))
    );
    assert_eq!(
        typed_value(
            &json!({"type": "decimal", "scale": 2, "unit": "usd"}),
            "4500"
        ),
        Some(json!({"type": "decimal", "coefficient": "4500", "scale": 2, "unit": "usd"}))
    );
    assert_eq!(
        typed_value(
            &json!({"type": "money", "currency": "USD", "minor_unit_exponent": 2}),
            "4500000"
        ),
        Some(
            json!({"type": "money", "minor_units": "4500000", "currency": "USD", "minor_unit_exponent": 2})
        )
    );
    assert_eq!(
        typed_value(
            &json!({"type": "reference", "target_input": "expenses"}),
            "00000000-0000-4000-8000-0000000000f0"
        ),
        Some(
            json!({"type": "reference", "input_id": "expenses", "record_id": "00000000-0000-4000-8000-0000000000f0"})
        )
    );
    assert_eq!(
        typed_value(&json!({"type": "boolean"}), "true"),
        Some(json!({"type": "boolean", "value": true}))
    );
    assert_eq!(
        typed_value(&json!({"type": "date"}), "2026-10-29"),
        Some(json!({"type": "date", "value": "2026-10-29"}))
    );
    assert_eq!(typed_value(&json!({"type": "integer"}), "12"), None);
    assert_eq!(typed_value(&json!({"type": "vector"}), "12"), None);
}

#[test]
fn notices_are_a_closed_set_read_only_from_the_notice_query() {
    for notice in [
        Notice::Committed,
        Notice::Replayed,
        Notice::Conflict,
        Notice::Stale,
        Notice::InFlight,
        Notice::Busy,
        Notice::Refused,
        Notice::Unknown,
    ] {
        assert_eq!(Notice::parse(notice.name()), Some(notice));
        assert_eq!(
            routes::notice_for_tests(Some(&format!("notice={}", notice.name()))),
            Some(notice)
        );
    }
    assert_eq!(Notice::parse("Committed"), None);
    assert_eq!(routes::notice_for_tests(None), None);
    assert_eq!(routes::notice_for_tests(Some("notice=committed&x=1")), None);
    assert_eq!(
        routes::notice_for_tests(Some("notice=committed&notice=stale")),
        None
    );
    assert_eq!(routes::notice_for_tests(Some("other=committed")), None);
}

#[tokio::test]
async fn conflict_problems_carry_the_recovery_view_as_canonical_json() {
    let view = RecoveryView {
        organization: "00000000-0000-4000-8000-0000000000c8".into(),
        app: "00000000-0000-4000-8000-0000000000c9".into(),
        work: "00000000-0000-4000-8000-0000000000cd".into(),
        publication: "00000000-0000-4000-8000-0000000000cc".into(),
        work_version: 1,
        record_id: "00000000-0000-4000-8000-0000000000f0".into(),
        command: "record_decision".into(),
        idempotency_key: "0123456789abcdef0123456789abcdef".into(),
        reason: ConflictReason::ExpectedVersion,
        expected_record_version: 0,
        current_record_version: 1,
        current_values: Some(
            json!([{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}}]),
        ),
        stored_outcome: None,
    };
    let response = http::problem_with(
        axum::http::StatusCode::CONFLICT,
        "conflict",
        json!({"recovery_view": view.document()}),
    );
    assert_eq!(response.status(), 409);
    let headers = response.headers().clone();
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let body = to_bytes(response.into_body(), 1 << 16).await.unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["status"], 409);
    assert_eq!(parsed["code"], "conflict");
    assert_eq!(parsed["recovery_view"]["kind"], "recovery_view");
    assert_eq!(parsed["recovery_view"]["reason"], "expected_version");
    assert_eq!(parsed["recovery_view"]["current_record_version"], 1);
    assert!(parsed["recovery_view"]["stored_outcome"].is_null());
    // Canonical bytes: ASCII-ordered keys, no insignificant whitespace.
    assert_eq!(body, crate::admission::encode(&parsed).unwrap());
    let shaped = CommandRequest {
        app: view.app.clone(),
        work: view.work.clone(),
        publication: view.publication.clone(),
        command: view.command.clone(),
        schema: "decision".into(),
        schema_version_expected: 1,
        record: view.record_id.clone(),
        expected_record_version: 0,
        expected_work_version: 1,
        values: json!([{"field_id": "status", "value": {"type": "enum", "value": "reviewed"}}]),
        idempotency_key: view.idempotency_key.clone(),
    };
    assert!(shaped.well_formed());
}
