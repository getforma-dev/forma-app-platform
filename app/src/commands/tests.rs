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
    storage::{self, CommandRequest, CommandResult, ConflictReason, CurrentRevision, RecoveryView},
};
use axum::{
    body::to_bytes,
    http::{header, StatusCode},
    response::Response,
};
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
    let view = recovery_view();
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

/// The edition's JSON Schema artifact and its command-route specimen.
const SCHEMA: &str = include_str!("../../../contracts/schema.json");
const RECOVERY: &str = include_str!("../../../contracts/fixtures/command-recovery.json");
/// Codes of the HTTP layer's own registry (`contracts/schema.py`: "other codes
/// belong to the HTTP layer's own registry"): the uniform 404, the
/// fetch-metadata and token refusal and the outage. They carry the same
/// `{status, code}` shape and nothing else.
const HTTP_LAYER: [&str; 3] = ["not_found", "cross_site", "unavailable"];

/// The Rust equivalent of `contracts/validate.py` `validate_problem` against
/// `schema.json` `$defs.problem`: canonical forma-json-v1 bytes, the closed
/// member set, `status` and `code` required, `status` equal to the HTTP
/// status, a registered code (or an HTTP-layer code with no other member),
/// and the edition's pairings (413, 429, the waiting codes' Retry-After, the
/// recovery view only on a 409 conflict, the current revision only on a 412
/// stale). The `Retry-After` header, when present, equals the body's member.
fn problem_conforms(status: u16, retry_after: Option<&str>, bytes: &[u8]) -> Result<(), String> {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let definition = &schema["$defs"]["problem"];
    let body = crate::admission::decode(bytes).map_err(|e| format!("not forma-json-v1: {e}"))?;
    if crate::admission::encode(&body).unwrap() != bytes {
        return Err("not canonical bytes".into());
    }
    let object = body.as_object().ok_or("not an object")?;
    let properties = definition["properties"].as_object().unwrap();
    assert_eq!(definition["additionalProperties"], json!(false));
    for name in object.keys() {
        if !properties.contains_key(name) {
            return Err(format!("member {name} is not in $defs.problem"));
        }
    }
    for name in definition["required"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if !object.contains_key(name) {
            return Err(format!("'{name}' is a required property"));
        }
    }
    let in_range = |value: &Value, spec: &Value| {
        value.as_i64().is_some_and(|v| {
            spec["minimum"].as_i64().is_none_or(|m| v >= m)
                && spec["maximum"].as_i64().is_none_or(|m| v <= m)
        })
    };
    if !in_range(&body["status"], &properties["status"]) || body["status"] != json!(status) {
        return Err(format!(
            "status {} is not the HTTP status {status}",
            body["status"]
        ));
    }
    let code = body["code"].as_str().ok_or("code is not text")?;
    let registered = properties["code"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == code);
    if !registered {
        if !(HTTP_LAYER.contains(&code) && object.len() == 2) {
            return Err(format!("code {code} is not registered"));
        }
        return Ok(());
    }
    if let Some(limit) = body.get("limit") {
        if !properties["limit"]["enum"]
            .as_array()
            .unwrap()
            .contains(limit)
        {
            return Err("limit is not registered".into());
        }
    }
    if let Some(seconds) = body.get("retry_after_seconds") {
        if !in_range(seconds, &properties["retry_after_seconds"]) {
            return Err("retry_after_seconds is not a positive safe integer".into());
        }
    }
    let has = |name: &str| object.contains_key(name);
    let require = |ok: bool, message: &str| {
        if ok {
            Ok(())
        } else {
            Err(message.to_owned())
        }
    };
    require(
        code != "payload_too_large" || status == 413,
        "payload_too_large is 413",
    )?;
    require(
        code != "limit_exceeded" || status == 429,
        "limit_exceeded is 429",
    )?;
    let names = code == "limit_exceeded" && has("limit") && has("retry_after_seconds");
    let waiting = code == "in_flight" || code == "busy";
    require(
        !waiting || (status == 409 && has("retry_after_seconds") && !has("limit")),
        "in_flight and busy are 409 with Retry-After seconds",
    )?;
    require(
        (status == 429) == names
            && has("limit") == names
            && has("retry_after_seconds") == (names || waiting),
        "a 429 names its limit and Retry-After seconds, and only a 429, in_flight or busy does",
    )?;
    require(
        (code == "conflict") == has("recovery_view") && (code != "conflict" || status == 409),
        "a conflict is 409 with the recovery view, and only a conflict carries one",
    )?;
    require(
        (code == "stale") == has("current_revision") && (code != "stale" || status == 412),
        "stale is 412 with the current revision, and only stale carries one",
    )?;
    if let Some(seconds) = body.get("retry_after_seconds") {
        require(
            retry_after == Some(seconds.to_string().as_str()),
            "the Retry-After header equals retry_after_seconds",
        )?;
    }
    Ok(())
}

async fn conforms(response: Response) -> Result<String, String> {
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    if headers[header::CONTENT_TYPE] != "application/problem+json" {
        return Err("not application/problem+json".into());
    }
    let retry_after = headers
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    problem_conforms(status, retry_after.as_deref(), &bytes)?;
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    Ok(body["code"].as_str().unwrap().to_owned())
}

fn recovery_view() -> RecoveryView {
    RecoveryView {
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
    }
}

#[test]
fn the_rust_problem_check_accepts_the_contract_specimen_and_refuses_its_negatives() {
    let specimen: Value = serde_json::from_str(RECOVERY).unwrap();
    for body in specimen["problems"].as_array().unwrap() {
        let bytes = crate::admission::encode(body).unwrap();
        let status = body["status"].as_u64().unwrap() as u16;
        let retry = body.get("retry_after_seconds").map(Value::to_string);
        assert_eq!(
            problem_conforms(status, retry.as_deref(), &bytes),
            Ok(()),
            "{body}"
        );
    }
    for (status, retry, body, expected) in [
        (
            409,
            Some("2"),
            json!({"code": "in_flight"}),
            "required property",
        ),
        (
            409,
            None,
            json!({"status": 409, "code": "in_flight"}),
            "Retry-After",
        ),
        (
            409,
            Some("2"),
            json!({"status": 409, "code": "busy", "retry_after_seconds": 2, "limit": "task_queue"}),
            "Retry-After",
        ),
        (
            400,
            None,
            json!({"code": "malformed_idempotency_key"}),
            "required property",
        ),
        (
            503,
            None,
            json!({"status": 503, "code": "unknown_outcome", "extra": 1}),
            "not in $defs.problem",
        ),
        (
            409,
            None,
            json!({"status": 409, "code": "conflict"}),
            "recovery view",
        ),
        (
            404,
            None,
            json!({"status": 404, "code": "not_found", "limit": "task_queue"}),
            "not registered",
        ),
        (
            400,
            None,
            json!({"status": 400, "code": "invented"}),
            "not registered",
        ),
        (
            400,
            None,
            json!({"status": 409, "code": "malformed_command"}),
            "HTTP status",
        ),
        (
            409,
            Some("3"),
            json!({"status": 409, "code": "busy", "retry_after_seconds": 2}),
            "header",
        ),
    ] {
        let error = problem_conforms(status, retry, &crate::admission::encode(&body).unwrap())
            .expect_err(&body.to_string());
        assert!(error.contains(expected), "{body}: {error}");
    }
}

/// APP-013 review M2: every problem body the command route, the outcome
/// lookup and the record page emit validates against edition
/// impl-2026-10-29.3's `$defs.problem` under `validate_problem`'s rules.
#[tokio::test]
async fn every_problem_body_the_command_routes_emit_validates_against_the_edition() {
    let current = CurrentRevision {
        publication: Some("00000000-0000-4000-8000-0000000000cc".into()),
        revision: Some("00000000-0000-4000-8000-0000000000cb".into()),
        definition_digest: Some(format!("sha256:{}", "7".repeat(64))),
        work_version: Some(1),
    };
    let mut emitted = Vec::new();
    for result in [
        Ok(CommandResult::Conflict(recovery_view())),
        Ok(CommandResult::Stale(current)),
        Ok(CommandResult::InFlight),
        Ok(CommandResult::Busy),
        Err(storage::Error::Denied),
        Err(storage::Error::UnknownOutcome),
        Err(storage::Error::Unavailable),
    ] {
        emitted.push(conforms(routes::json_answer_for_tests(result)).await);
    }
    // The route's own refusals before storage (`routes::submit`), and the
    // outcome lookup's and record page's 404 and 503.
    for response in [
        http::cross_site(),
        http::not_found(),
        http::payload_too_large(),
        http::unavailable(),
        http::problem(StatusCode::BAD_REQUEST, "missing_idempotency_key"),
        http::problem(StatusCode::BAD_REQUEST, "malformed_idempotency_key"),
        http::problem(StatusCode::BAD_REQUEST, "malformed_command"),
    ] {
        emitted.push(conforms(response).await);
    }
    let mut codes: Vec<String> = emitted
        .into_iter()
        .map(|r| r.unwrap_or_else(|e| panic!("{e}")))
        .collect();
    codes.sort();
    codes.dedup();
    assert_eq!(
        codes,
        [
            "busy",
            "conflict",
            "cross_site",
            "in_flight",
            "malformed_command",
            "malformed_idempotency_key",
            "missing_idempotency_key",
            "not_found",
            "payload_too_large",
            "stale",
            "unavailable",
            "unknown_outcome",
        ]
    );
}

/// APP-013 review m2: a form submission storage refuses returns to the
/// record page with the refused notice (a live session that loaded the page
/// learns nothing new from it); the other form answers are unchanged.
#[test]
fn a_refused_form_submission_redirects_to_the_record_page_with_the_refused_notice() {
    let page = "/work/a/w/records/c/r?notice=";
    for (result, notice) in [
        (Err(storage::Error::Denied), Some("refused")),
        (Ok(CommandResult::InFlight), Some("in_flight")),
        (Ok(CommandResult::Busy), Some("busy")),
        (Err(storage::Error::UnknownOutcome), Some("unknown")),
        (Err(storage::Error::Unavailable), None),
    ] {
        let response = routes::form_answer_for_tests(result);
        match notice {
            Some(notice) => {
                assert_eq!(response.status(), StatusCode::SEE_OTHER);
                assert_eq!(
                    response.headers()[header::LOCATION],
                    format!("{page}{notice}").as_str()
                );
            }
            None => assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE),
        }
    }
}

/// APP-013 review m1 and m4: the JSON body is decoded once as forma-json-v1
/// (duplicate members, floats and non-JSON are `malformed_command`, reported
/// before any key question), and the `Idempotency-Key` header is exactly one
/// value used verbatim (two headers are `malformed_idempotency_key`; a padded
/// value is not trimmed, so the key-shape check refuses it).
#[test]
fn json_requests_decode_once_strictly_and_take_exactly_one_verbatim_header_key() {
    use axum::http::{HeaderMap, HeaderValue};
    let (command, _) = fixture_request();
    let (app, work, id) = (
        command["app_id"].as_str().unwrap().to_owned(),
        command["work_id"].as_str().unwrap().to_owned(),
        command["command_id"].as_str().unwrap().to_owned(),
    );
    let key = "0123456789abcdef0123456789abcdef";
    let headers = |values: &[&str]| {
        let mut map = HeaderMap::new();
        for value in values {
            map.append("idempotency-key", HeaderValue::from_str(value).unwrap());
        }
        map
    };
    let parse = |map: &HeaderMap, bytes: &[u8]| {
        routes::json_request_for_tests(map, bytes, &app, &work, &id)
    };
    let bytes = serde_json::to_vec(&command).unwrap();
    assert_eq!(
        parse(&headers(&[key]), &bytes).map(|r| r.idempotency_key),
        Ok(key.to_owned())
    );
    // Duplicate members: a top-level one and one inside a value object.
    let text = String::from_utf8(bytes.clone()).unwrap();
    let duplicated = format!("{{\"schema_version\":1,{}", &text[1..]);
    assert_eq!(
        parse(&headers(&[key]), duplicated.as_bytes()).err(),
        Some("malformed_command")
    );
    let inner = text.replacen(
        "\"value\":\"needs_context\"",
        "\"value\":\"reviewed\",\"value\":\"needs_context\"",
        1,
    );
    assert_ne!(inner, text);
    assert_eq!(
        parse(&headers(&[key]), inner.as_bytes()).err(),
        Some("malformed_command")
    );
    // serde_json's last-wins reading is what the old route judged.
    assert!(serde_json::from_str::<Value>(&duplicated).is_ok());
    // A non-JSON body without a key header is malformed, not keyless.
    assert_eq!(
        parse(&headers(&[]), b"not json").err(),
        Some("malformed_command")
    );
    assert_eq!(
        parse(&headers(&[]), b"{\"kind\":1.5}").err(),
        Some("malformed_command")
    );
    // A JSON body without any key is missing it.
    let mut keyless = command.clone();
    keyless.as_object_mut().unwrap().remove("idempotency_key");
    let keyless = serde_json::to_vec(&keyless).unwrap();
    assert_eq!(
        parse(&headers(&[]), &keyless).err(),
        Some("missing_idempotency_key")
    );
    // Two headers refuse, even when they agree.
    assert_eq!(
        parse(&headers(&[key, key]), &keyless).err(),
        Some("malformed_idempotency_key")
    );
    // A padded header is used verbatim and then fails the key shape.
    let padded = parse(&headers(&[&format!(" {key} ")]), &keyless).unwrap();
    assert_eq!(padded.idempotency_key, format!(" {key} "));
    assert!(!crate::storage::idempotency_key_shaped(
        &padded.idempotency_key
    ));
}

/// APP-013 review M1 (the page's command choice) and m3: the record page
/// offers the first declared command storage would admit on the record's
/// state — a `create` on an absent record; on an existing one an `update`,
/// or a `create` one of whose declared transitions starts at the record's
/// current value — and no form when none would; only the offered command's
/// writable fields are inputs.
#[test]
fn the_record_page_offers_only_a_command_storage_would_admit_and_only_its_writable_fields() {
    use crate::storage::{RecordCommand, RecordDetail, RecordField};
    let status = |value: Option<&str>| RecordField {
        id: "status".into(),
        value_type: json!({"type": "enum", "values": ["needs_context", "reviewed"]}),
        nullable: false,
        value: value.map_or(
            json!({"type": "null"}),
            |v| json!({"type": "enum", "value": v}),
        ),
        writable: false,
    };
    let note = RecordField {
        id: "note".into(),
        value_type: json!({"type": "text"}),
        nullable: true,
        value: json!({"type": "null"}),
        writable: false,
    };
    let record_decision = RecordCommand {
        id: "record_decision".into(),
        operation: "create".into(),
        writable_fields: vec!["status".into()],
        transitions: vec![("status".into(), "needs_context".into(), "reviewed".into())],
    };
    let detail = |version: i64, current: Option<&str>, commands: Vec<RecordCommand>| RecordDetail {
        record_id: "00000000-0000-4000-8000-0000000000f0".into(),
        collection: "00000000-0000-4000-8000-0000000000ce".into(),
        schema: "decision".into(),
        schema_version: 1,
        publication: "00000000-0000-4000-8000-0000000000cc".into(),
        definition_digest: format!("sha256:{}", "7".repeat(64)),
        work_version: 1,
        version,
        fields: vec![status(current), note.clone()],
        commands,
        can_write: true,
    };
    let offered = |detail: &RecordDetail| {
        let view = super::view("a", "w", detail, "t".into(), "k".into(), None);
        let writable: Vec<(String, bool)> = view
            .fields
            .iter()
            .map(|row| (row.id.clone(), row.writable))
            .collect();
        (view.command, view.can_write, writable)
    };
    let only_status = vec![("status".into(), true), ("note".into(), false)];
    let none = vec![("status".into(), false), ("note".into(), false)];
    // Absent record: the create command.
    assert_eq!(
        offered(&detail(0, None, vec![record_decision.clone()])),
        ("record_decision".into(), true, only_status.clone())
    );
    // needs_context: the declared transition can fire.
    assert_eq!(
        offered(&detail(
            1,
            Some("needs_context"),
            vec![record_decision.clone()]
        )),
        ("record_decision".into(), true, only_status.clone())
    );
    // reviewed: no declared transition starts there, so no form.
    assert_eq!(
        offered(&detail(2, Some("reviewed"), vec![record_decision.clone()])),
        (String::new(), false, none.clone())
    );
    // A create without transitions is never offered on an existing record;
    // a declared update is.
    let mut plain = record_decision.clone();
    plain.transitions.clear();
    let update = RecordCommand {
        id: "edit_note".into(),
        operation: "update".into(),
        writable_fields: vec!["note".into()],
        transitions: Vec::new(),
    };
    assert_eq!(
        offered(&detail(2, Some("reviewed"), vec![plain.clone()])),
        (String::new(), false, none)
    );
    assert_eq!(
        offered(&detail(2, Some("reviewed"), vec![plain, update.clone()])),
        (
            "edit_note".into(),
            true,
            vec![("status".into(), false), ("note".into(), true)]
        )
    );
    // An update is never offered on an absent record; a reader without
    // record_write gets no form.
    assert_eq!(offered(&detail(0, None, vec![update])).0, "");
    let mut reader = detail(1, Some("needs_context"), vec![record_decision]);
    reader.can_write = false;
    assert!(!offered(&reader).1);
}
