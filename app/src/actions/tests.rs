//! Unit tests of the action path's typed seams: the closed parse shape, the
//! rehearsed parser, the money and approver resolution, the request body.
use super::{
    decode,
    prompt::{CatalogApp, CatalogCommand},
    rehearsed,
    resolve::{cents, display, match_approver, match_target},
    routes::{body, Body},
    Kind,
};
use crate::storage::{Member, RecordRow};
use serde_json::{json, Value};

fn expense_app() -> CatalogApp {
    let field = |id: &str, spec: Value| (id.to_owned(), spec);
    CatalogApp {
        app: "00000000-0000-4000-8000-000000001681".into(),
        title: "Expense claims".into(),
        purpose: "File an expense".into(),
        schema: "expense_claim".into(),
        fields: vec![
            field("amount", json!({"type": "money"})),
            field("merchant", json!({"type": "text"})),
            field(
                "category",
                json!({"type": "enum", "values": ["meals", "travel", "software", "office", "other"]}),
            ),
            field("spent_on", json!({"type": "date"})),
            field("note", json!({"type": "text", "nullable": true})),
            field(
                "status",
                json!({"type": "enum", "values": ["submitted", "approved", "rejected"]}),
            ),
        ],
        commands: vec![
            CatalogCommand {
                id: "submit".into(),
                operation: "create".into(),
                writable: [
                    "amount", "merchant", "category", "spent_on", "note", "status",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                transitions: vec![],
            },
            CatalogCommand {
                id: "decide".into(),
                operation: "create".into(),
                writable: vec!["status".into(), "note".into()],
                transitions: vec![
                    ("status".into(), "submitted".into(), "approved".into()),
                    ("status".into(), "submitted".into(), "rejected".into()),
                ],
            },
        ],
    }
}

#[test]
fn the_parse_shape_is_closed_and_hints_are_bounded() {
    let parsed = decode(
        r#"Sure! {"action":"create","app_hint":"00000000-0000-4000-8000-000000001681","values":{"amount":"86","merchant":"Nobu","category":"meals","spent_on":"2026-10-29","note":"client dinner"},"approver_name":"Sarah","target_record_hint":null}"#,
    )
    .unwrap();
    assert_eq!(parsed.kind, Kind::Create);
    assert_eq!(parsed.app_hint, "00000000-0000-4000-8000-000000001681");
    assert_eq!(parsed.values["amount"], "86");
    assert_eq!(parsed.approver_name.as_deref(), Some("Sarah"));
    assert!(parsed.target_record_hint.is_none());
    // An unknown member, an unknown action, a non-object: refused.
    assert!(decode(r#"{"action":"create","grant":"admin_policy"}"#).is_none());
    assert!(decode(r#"{"action":"delete_everything"}"#).is_none());
    assert!(decode("[1,2]").is_none());
    assert!(decode("no json here").is_none());
    // A key outside the identifier grammar is refused by the strict decoder
    // itself; an invalid app hint is dropped and a numeric value kept as text.
    assert!(decode(r#"{"action":"decide","values":{"BadField":"1"}}"#).is_none());
    let loose = decode(r#"{"action":"decide","app_hint":"DROP TABLE","values":{"status":"approved","amount":86},"target_record_hint":"the Nobu dinner"}"#).unwrap();
    assert_eq!(loose.kind, Kind::Decide);
    assert_eq!(loose.app_hint, "");
    assert_eq!(loose.values["amount"], "86");
    assert_eq!(loose.target_record_hint.as_deref(), Some("the Nobu dinner"));
}

#[test]
fn the_rehearsed_parser_answers_the_four_scenario_utterances_and_nothing_else() {
    let catalog = vec![expense_app()];
    let today = "2026-10-30";
    let create = rehearsed::parse(
        "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses",
        &catalog,
        today,
    )
    .unwrap();
    assert_eq!(create.kind, Kind::Create);
    assert_eq!(create.values["amount"], "86");
    assert_eq!(create.values["merchant"], "Nobu");
    assert_eq!(create.values["category"], "meals");
    assert_eq!(create.values["spent_on"], "2026-10-29");
    assert_eq!(create.values["note"], "a client dinner");
    assert_eq!(create.values["status"], "submitted");
    assert_eq!(create.approver_name.as_deref(), Some("Sarah"));
    let decide = rehearsed::parse("approve the Nobu dinner", &catalog, today).unwrap();
    assert_eq!(decide.kind, Kind::Decide);
    assert_eq!(decide.values["status"], "approved");
    assert_eq!(decide.target_record_hint.as_deref(), Some("Nobu dinner"));
    let reject = rehearsed::parse("Reject Victor's hotel", &catalog, today).unwrap();
    assert_eq!(reject.values["status"], "rejected");
    let open = rehearsed::parse("what's waiting for my approval?", &catalog, today).unwrap();
    assert_eq!(open.kind, Kind::Open);
    assert_eq!(open.app_hint, catalog[0].app);
    // Unrehearsed text is the model's.
    assert!(rehearsed::parse(
        "Investigate outbound traffic from build-worker-17",
        &catalog,
        today
    )
    .is_none());
    assert!(rehearsed::parse("Build a PTO request app with approvals", &catalog, today).is_none());
    // Variants: a two-word approver, "send it to", a bare currency word.
    let two = rehearsed::parse(
        "Paid 42.50 dollars for a Figma subscription yesterday; send it to Sarah Chen",
        &catalog,
        today,
    )
    .unwrap();
    assert_eq!(two.values["amount"], "42.50");
    assert_eq!(two.values["category"], "software");
    assert_eq!(two.values["spent_on"], "2026-10-29");
    assert_eq!(two.approver_name.as_deref(), Some("Sarah Chen"));
    assert_eq!(
        rehearsed::amount("costs $1,234.5 total"),
        Some("1234.5".into())
    );
    assert_eq!(rehearsed::amount("room 86"), None);
    assert_eq!(
        rehearsed::days_before("2026-03-01", 1),
        Some("2026-02-28".into())
    );
    assert_eq!(
        rehearsed::days_before("2026-01-01", 1),
        Some("2025-12-31".into())
    );
}

#[test]
fn money_resolves_to_cents_and_displays_as_dollars() {
    assert_eq!(cents("86"), Some("8600".into()));
    assert_eq!(cents("$86.5"), Some("8650".into()));
    assert_eq!(cents("1,234.00"), Some("123400".into()));
    assert_eq!(cents("0.07"), Some("7".into()));
    assert_eq!(cents("0"), Some("0".into()));
    assert_eq!(cents("12.345"), None);
    assert_eq!(cents("abc"), None);
    assert_eq!(cents(""), None);
    assert_eq!(
        display(
            &json!({"type": "money", "minor_units": "8600", "currency": "USD", "minor_unit_exponent": 2})
        ),
        "$86.00"
    );
    assert_eq!(
        display(
            &json!({"type": "money", "minor_units": "123456789", "currency": "USD", "minor_unit_exponent": 2})
        ),
        "$1,234,567.89"
    );
    assert_eq!(
        display(
            &json!({"type": "money", "minor_units": "7", "currency": "USD", "minor_unit_exponent": 2})
        ),
        "$0.07"
    );
    assert_eq!(display(&json!({"type": "null"})), "—");
    assert_eq!(
        display(&json!({"type": "enum", "value": "approved"})),
        "approved"
    );
}

#[test]
fn approvers_match_display_names_only_and_targets_match_record_words() {
    let sarah = "00000000-0000-4000-8000-00000000006d";
    let sam = "00000000-0000-4000-8000-0000000000ca";
    let unnamed = "00000000-0000-4000-8000-00000000006f";
    crate::directory::set_for_tests(&[(sarah, "Sarah Chen"), (sam, "Samuel Okafor")]);
    let members: Vec<Member> = [sarah, sam, unnamed]
        .iter()
        .map(|p| Member {
            principal: (*p).to_owned(),
            label: crate::directory::label(p),
        })
        .collect();
    assert_eq!(match_approver("Sarah", &members).unwrap().principal, sarah);
    assert_eq!(
        match_approver("sarah chen", &members).unwrap().label,
        "Sarah Chen"
    );
    assert_eq!(match_approver("Sam", &members).unwrap().principal, sam);
    assert!(
        match_approver("Member", &members).is_none(),
        "a neutral label never matches a name"
    );
    assert!(match_approver("Priya", &members).is_none());
    crate::directory::set_for_tests(&[]);

    let record = |id: &str, merchant: &str, cents: &str, status: &str, sequence: i64| RecordRow {
        record_id: id.into(),
        collection: "c".into(),
        schema: "expense_claim".into(),
        version: 1,
        commit_sequence: sequence,
        committed_at: "2026-10-30T10:00:00Z".into(),
        creator: "00000000-0000-4000-8000-000000000066".into(),
        updater: "00000000-0000-4000-8000-000000000066".into(),
        values: json!([
            {"field_id": "amount", "value": {"type": "money", "minor_units": cents, "currency": "USD", "minor_unit_exponent": 2}},
            {"field_id": "merchant", "value": {"type": "text", "value": merchant}},
            {"field_id": "status", "value": {"type": "enum", "value": status}},
        ]),
    };
    let records = vec![
        record(
            "00000000-0000-4000-8000-0000000000a3",
            "Hilton",
            "21000",
            "submitted",
            9,
        ),
        record(
            "00000000-0000-4000-8000-0000000000a2",
            "Nobu",
            "8600",
            "submitted",
            8,
        ),
        record(
            "00000000-0000-4000-8000-0000000000a1",
            "Nobu",
            "9900",
            "approved",
            7,
        ),
    ];
    let hit = match_target(Some("the Nobu dinner"), &records, "status", &["submitted"]).unwrap();
    assert_eq!(
        hit.record_id, "00000000-0000-4000-8000-0000000000a2",
        "the pending Nobu one, not the approved one"
    );
    let newest = match_target(None, &records, "status", &["submitted"]).unwrap();
    assert_eq!(newest.record_id, "00000000-0000-4000-8000-0000000000a3");
    let by_amount = match_target(Some("the $86 one"), &records, "status", &["submitted"]).unwrap();
    assert_eq!(by_amount.record_id, "00000000-0000-4000-8000-0000000000a2");
    let by_id = match_target(
        Some("00000000-0000-4000-8000-0000000000a3"),
        &records,
        "status",
        &["submitted"],
    )
    .unwrap();
    assert_eq!(by_id.record_id, "00000000-0000-4000-8000-0000000000a3");
    assert!(match_target(Some("x"), &records, "status", &["rejected"]).is_none());
}

#[test]
fn the_request_body_is_an_utterance_or_a_structured_decision() {
    assert!(matches!(
        body(br#"{"utterance":"I spent $86 at Nobu"}"#),
        Some(Body::Utterance(_))
    ));
    assert!(body(br#"{"utterance":"ab"}"#).is_none());
    assert!(body(br#"{"utterance":"abc","extra":1}"#).is_none());
    assert!(body(b"[]").is_none());
    let decide = body(br#"{"decide":{"app_id":"00000000-0000-4000-8000-000000001681","work_id":"00000000-0000-4000-8000-000000001684","record_id":"00000000-0000-4000-8000-0000000000a2","status":"approved","note":" ok "}}"#).unwrap();
    match decide {
        Body::Decide { status, note, .. } => {
            assert_eq!(status, "approved");
            assert_eq!(note.as_deref(), Some("ok"));
        }
        Body::Utterance(_) => panic!("structured"),
    }
    assert!(body(
        br#"{"decide":{"app_id":"x","work_id":"y","record_id":"z","status":"approved"}}"#
    )
    .is_none());
    assert!(body(br#"{"decide":{"app_id":"00000000-0000-4000-8000-000000001681","work_id":"00000000-0000-4000-8000-000000001684","record_id":"00000000-0000-4000-8000-0000000000a2","status":"approved","permission":"admin"}}"#).is_none());
}
