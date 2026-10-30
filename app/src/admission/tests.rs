//! APP-010 admission against the locked contract edition impl-2026-10-29.2 and
//! the edition impl-2026-10-28.1 it retains (ADR-009 D9.6). Every expected byte
//! string, digest and verdict here is locked in
//! `contracts/implementation-lock.json` or was produced by the Python reference
//! (`contracts/validate.py` on Python 3.11.0) from the same input.
use super::canonical::{decode, encode, MAX_DOCUMENT_BYTES};
use super::digest::{self, digest, DOMAINS};
use super::validate::{
    admit_definition, check_exact_rational, exact_rational, AdmittedDefinition,
    MAX_DEFINITION_BYTES,
};
use super::AdmissionError::{self, *};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const LOCK: &str = include_str!("../../../contracts/implementation-lock.json");
/// Every locked `contracts/fixtures/**/*.json` artifact: edition three's
/// specimen, the retained edition two's, the retained edition one's and the
/// current bundles and vectors (edition two's, unchanged by edition three).
const FIXTURES: [(&str, &str); 13] = [
    (
        "contracts/fixtures/command-recovery.json",
        include_str!("../../../contracts/fixtures/command-recovery.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/canonical-vectors.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-29.2/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/negative-vectors.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-29.2/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/northstar.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-29.2/northstar.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-29.2/spend-review.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-29.2/spend-review.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/negative-vectors.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-28.1/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/northstar.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-28.1/northstar.json"),
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/spend-review.json",
        include_str!("../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json"),
    ),
    (
        "contracts/fixtures/canonical-vectors.json",
        include_str!("../../../contracts/fixtures/canonical-vectors.json"),
    ),
    (
        "contracts/fixtures/negative-vectors.json",
        include_str!("../../../contracts/fixtures/negative-vectors.json"),
    ),
    (
        "contracts/fixtures/northstar.json",
        include_str!("../../../contracts/fixtures/northstar.json"),
    ),
    (
        "contracts/fixtures/spend-review.json",
        include_str!("../../../contracts/fixtures/spend-review.json"),
    ),
];

/// Raw input, canonical bytes and `forma:result:v1` digest from validate.py
/// `loads` -> `canonical` -> `digest("result", ...)`.
const PYTHON_DECODED: [(&[u8], &[u8], &str); 13] = [
    (
        b"-0",
        b"0",
        "sha256:60a429cf9ea639c44a6ff1a91b7cba31e1387f9604a4d2137b72d716c1ba14ba",
    ),
    (
        br#"{"a":-0}"#,
        br#"{"a":0}"#,
        "sha256:7f2efea68844b3dcde5b6bef8fcb15704968aca21492a9354bbeb19e6aea694f",
    ),
    (
        b" \t\n\r{ \"b\" : \"\\u00e9\\/x\" , \"a\" : [ ] }\r\n",
        "{\"a\":[],\"b\":\"\u{e9}/x\"}".as_bytes(),
        "sha256:f45485548a3596d0490cca3686092faf8bfd17a0dc463a5110609deb10cd41b0",
    ),
    (
        b"[1,\"\\u0000\\u001f\\u007f\\u2028\\u2029\"]",
        "[1,\"\\u0000\\u001f\u{7f}\u{2028}\u{2029}\"]".as_bytes(),
        "sha256:45364588df7c68382106e7cf6aed83046a95bb6ef1bf24a4557af98c07b474c9",
    ),
    (
        b"{\"a\":\"\\uD83D\\uDE00\"}",
        "{\"a\":\"\u{1f600}\"}".as_bytes(),
        "sha256:75d437b526b84af7fa384c894324ef1a0667e7925eab150f5d45a62798055396",
    ),
    (
        br#""\b\f\n\r\t\"\\""#,
        br#""\b\f\n\r\t\"\\""#,
        "sha256:99f18ebb9d41ddc9d6d7c553b4f711f0af8b598f13995ce1c91e2ecc1637ae93",
    ),
    (
        b"-9007199254740991",
        b"-9007199254740991",
        "sha256:738a523248b4b9888842fda4833bdc607d770280ec13cfb4568b5c1377212024",
    ),
    (
        "\"\u{7f}\u{2028}\u{e9}\"".as_bytes(),
        "\"\u{7f}\u{2028}\u{e9}\"".as_bytes(),
        "sha256:7d39cb21feeaca0ecbc103e8ae72a8bf2f0c8e3785b43cb2495315899e48149b",
    ),
    (
        b"\"\\u00E9\"",
        "\"\u{e9}\"".as_bytes(),
        "sha256:1443c8fa6a5ae9ac6f2998abb705d3988a7346d84e4ebc56ae34ed0a1e2c75de",
    ),
    (
        br#""\/""#,
        br#""/""#,
        "sha256:082e12e30e3b8ecb5c8e294e1041d3218727e881eedf2067379558b305065f3b",
    ),
    (
        br#"{"a":{"b":{"c":[true,false,null]}}}"#,
        br#"{"a":{"b":{"c":[true,false,null]}}}"#,
        "sha256:5db119106ed254be30677443804e77a63416003a278557ca65f9c0c05bd399da",
    ),
    (
        b"[]",
        b"[]",
        "sha256:c5ddb58161fec738e5c78533fee005fa7a9e6f6c64808dc5ff5b2b77ff39156d",
    ),
    (
        b"\"\"",
        b"\"\"",
        "sha256:d49bf9a61361709ce8254950e0f687a780e62e9c54a2cbc77667800b2f2df8c4",
    ),
];

/// Inputs validate.py `loads` refuses: duplicate keys, floats, non-finite and
/// unsafe numbers, invalid keys, lone surrogates, malformed UTF-8 and syntax.
const PYTHON_REFUSED: [&[u8]; 55] = [
    br#"{"a":1,"a":2}"#,
    b"{\"a\":1,\"\\u0061\":2}",
    br#"{"a":1.0}"#,
    br#"{"a":1e2}"#,
    br#"{"a":1E2}"#,
    br#"{"a":-0.0}"#,
    br#"{"a":0e0}"#,
    br#"{"a":NaN}"#,
    br#"{"a":Infinity}"#,
    br#"{"a":-Infinity}"#,
    br#"{"a":9007199254740992}"#,
    br#"{"a":-9007199254740992}"#,
    br#"{"a":18446744073709551616}"#,
    "{\"\u{e9}\":1}".as_bytes(),
    br#"{"A":1}"#,
    br#"{"_a":1}"#,
    br#"{"1a":1}"#,
    br#"{"":1}"#,
    br#"{"a-b":1}"#,
    br#"{"a":"\ud800"}"#,
    br#"{"a":"\udc00"}"#,
    b"{\"a\":\"\\ud800\\u0041\"}",
    b"{} {}",
    b"{}x",
    b"\"\x01\"",
    b"{\"a\":\"\t\"}",
    b"{\"a\":\"\n\"}",
    b"\xff",
    b"\"\xc0\xaf\"",
    b"\"\xed\xa0\x80\"",
    b"\xef\xbb\xbf{}",
    b"",
    b"   ",
    b"01",
    b"-01",
    b"+1",
    b".5",
    b"1.",
    b"-",
    b"{'a':1}",
    b"[1,]",
    br#"{"a":1,}"#,
    b"/*x*/{}",
    br#""\x""#,
    br#""\u12""#,
    b"tru",
    b"nulls",
    b"1 2",
    b"[1 2]",
    br#"{"a" 1}"#,
    br#"{"a":}"#,
    b"{,}",
    b"[,1]",
    b"[",
    b"\"open",
];

/// `{}` in every registered domain, from validate.py `digest(name, {})`.
const DOMAIN_DIGESTS: [(&str, &str); 10] = [
    (
        "forma:app-definition:v1",
        "sha256:084dfaff00f2bb828c01a6262d2fa4e3d1cb8178151511976d85c73868b2ed3b",
    ),
    (
        "forma:input-contract:v1",
        "sha256:b5fb8b656f511b895faac99c14d8253ef1398df8a6b61c20f31a5e12e84497dc",
    ),
    (
        "forma:record-schema:v1",
        "sha256:69c5f29562247798337dcb86e0ec44493c5a5c6f1ffe7f2bd8a8da1c87673911",
    ),
    (
        "forma:query:v1",
        "sha256:369c7411b19d745c2d02627f52db49bd2f7a9346565272b5a3edb37613463e22",
    ),
    (
        "forma:query-receipt:v1",
        "sha256:f2aefe4425b566e5af1d536b0554e9073b212717cd341e6ce933afe4bf3b3b31",
    ),
    (
        "forma:command-payload:v1",
        "sha256:d8d7d202bb24bafc0893966873614c741623de7e35c7433ca523bdf8182e3e1b",
    ),
    (
        "forma:source-binding:v1",
        "sha256:dfa8446eead07cca96294e581e60a1b3f998514714b8a00c9634e5856887fb6e",
    ),
    (
        "forma:result:v1",
        "sha256:e64268741d1dcc2933132abf459c5f66d635773d21489a604aa7affda1fd5e40",
    ),
    (
        "forma:normalized-rows:v1",
        "sha256:88d78f5aefbf22c671780bc9b8b33a3c4056cf1530ad15133af4fcc77ad919ae",
    ),
    (
        "forma:job-payload:v1",
        "sha256:6543802750686a3a2c6de5e84c01ecd2f73199bcf17c36315f968478fae2eb44",
    ),
];

/// Every registered value type, literal, operator and renderer kind in one
/// definition content; validate.py admits it with [`RICH_DIGEST`].
const RICH_CONTENT: &str = r#"{
  "title": "Every registered shape",
  "description": "Synthetic admission coverage; free text such as <b>bold</b>; SELECT 1; https://example.invalid/x stays data.",
  "inputs": [
    {"id":"events","contract_id":"events_v1","contract_version":1,"kind":"source","fields":[{"id":"time","value_type":{"type":"timestamp"},"nullable":false},{"id":"destination","value_type":{"type":"text"},"nullable":false},{"id":"bytes","value_type":{"type":"integer","unit":"byte"},"nullable":false},{"id":"cost","value_type":{"type":"decimal","unit":"usd","scale":2},"nullable":false},{"id":"amount","value_type":{"type":"money","currency":"USD","minor_unit_exponent":2},"nullable":false},{"id":"region","value_type":{"type":"enum","values":["east","west"]},"nullable":false},{"id":"related","value_type":{"type":"reference","target_input":"inventory"},"nullable":false},{"id":"day","value_type":{"type":"date"},"nullable":false},{"id":"flagged","value_type":{"type":"boolean"},"nullable":true}]},
    {"id":"inventory","contract_id":"inventory_v1","contract_version":1,"kind":"source","fields":[{"id":"address","value_type":{"type":"text"},"nullable":false},{"id":"label","value_type":{"type":"text"},"nullable":false}]},
    {"id":"decisions","contract_id":"decisions_v1","contract_version":1,"kind":"records","fields":[{"id":"status","value_type":{"type":"enum","values":["needs_context","reviewed"]},"nullable":false},{"id":"note","value_type":{"type":"text"},"nullable":false}]}
  ],
  "views": [
    {"id":"total","kind":"summary","renderer_version":1,"title":"Total","query":{"operator_version":1,"inputs":["events"],"operations":[{"type":"filter","field":{"input_id":"events","field_id":"flagged"},"comparison":"eq","value":{"type":"boolean","value":true}},{"type":"aggregate","group_by":[{"input_id":"events","field_id":"region"}],"measures":[{"id":"total_bytes","operation":"sum","field":{"input_id":"events","field_id":"bytes"}},{"id":"rows","operation":"count","field":{"input_id":"events","field_id":"destination"}}]},{"type":"ratio","numerator":"total_bytes","denominator":{"type":"measure","id":"rows"},"output_field":"bytes_per_row","output_unit":"byte","zero_denominator":"error","rounding":"exact_rational"},{"type":"sort","fields":[{"field":{"input_id":"events","field_id":"region"},"direction":"asc"}],"stable_tiebreak":{"input_id":"events","field_id":"region"}},{"type":"limit","rows":500}],"null_policy":"unknown","window":{"start":"2026-10-01T00:00:00Z","end":"2026-11-01T00:00:00Z","bounds":"half_open"}}},
    {"id":"destinations","kind":"table","renderer_version":1,"title":"Destinations","query":{"operator_version":1,"inputs":["events","inventory"],"operations":[{"type":"lookup","left":{"input_id":"events","field_id":"destination"},"right":{"input_id":"inventory","field_id":"address"},"rule":"cidr_exact_then_longest","cardinality":"at_most_one"},{"type":"project","fields":[{"input_id":"events","field_id":"destination"},{"input_id":"events","field_id":"bytes"}]},{"type":"filter","field":{"input_id":"events","field_id":"region"},"comparison":"eq","value":{"type":"enum","value":"east"}},{"type":"filter","field":{"input_id":"events","field_id":"day"},"comparison":"gte","value":{"type":"date","value":"2024-02-29"}},{"type":"filter","field":{"input_id":"events","field_id":"time"},"comparison":"lt","value":{"type":"timestamp","value":"2026-10-28T23:59:59Z"}},{"type":"filter","field":{"input_id":"events","field_id":"cost"},"comparison":"gte","value":{"type":"decimal","coefficient":"12345","scale":2,"unit":"usd"}},{"type":"filter","field":{"input_id":"events","field_id":"amount"},"comparison":"gte","value":{"type":"money","minor_units":"20000","currency":"USD","minor_unit_exponent":2}},{"type":"filter","field":{"input_id":"events","field_id":"destination"},"comparison":"ne","value":{"type":"text","value":"<script>alert(1)</script>; DROP TABLE x; ../../etc/passwd"}}],"null_policy":"unknown"}},
    {"id":"timeline","kind":"timeline","renderer_version":1,"title":"Timeline","query":{"operator_version":1,"inputs":["events"],"operations":[{"type":"time_bucket","field":{"input_id":"events","field_id":"time"},"seconds":3600,"timezone":"UTC","output_field":"hour"},{"type":"aggregate","group_by":[],"measures":[{"id":"hourly_bytes","operation":"max","field":{"input_id":"events","field_id":"bytes"}}]},{"type":"ratio","numerator":"hourly_bytes","denominator":{"type":"constant","value":"1024","unit":"byte"},"output_field":"hourly_kib","output_unit":"kib","zero_denominator":"error","rounding":"exact_rational"}],"null_policy":"unknown"}},
    {"id":"related","kind":"detail","renderer_version":1,"title":"Related","query":{"operator_version":1,"inputs":["events"],"operations":[{"type":"filter","field":{"input_id":"events","field_id":"related"},"comparison":"eq","value":{"type":"reference","input_id":"inventory","record_id":"00000000-0000-7000-a000-000000000001"}},{"type":"filter","field":{"input_id":"events","field_id":"bytes"},"comparison":"gt","value":{"type":"integer","value":"-5","unit":"byte"}},{"type":"filter","field":{"input_id":"events","field_id":"flagged"},"comparison":"ne","value":{"type":"null"}},{"type":"project","fields":[{"input_id":"events","field_id":"related"}]}],"null_policy":"unknown"}},
    {"id":"spend","kind":"chart","renderer_version":1,"title":"Spend","query":{"operator_version":1,"inputs":["events"],"operations":[{"type":"aggregate","group_by":[{"input_id":"events","field_id":"day"}],"measures":[{"id":"spend","operation":"sum","field":{"input_id":"events","field_id":"amount"}},{"id":"cheapest","operation":"min","field":{"input_id":"events","field_id":"cost"}}]}],"null_policy":"unknown"}},
    {"id":"regions","kind":"filter","renderer_version":1,"title":"Regions","query":{"operator_version":1,"inputs":["events"],"operations":[{"type":"project","fields":[{"input_id":"events","field_id":"region"}]}],"null_policy":"unknown"}},
    {"id":"record","kind":"form","renderer_version":1,"title":"Record decision","command_id":"record_decision"},
    {"id":"caution","kind":"note","renderer_version":1,"title":"Caution","text":"An unmatched destination does not establish compromise."},
    {"id":"status","kind":"status","renderer_version":1,"title":"Status","query":{"operator_version":1,"inputs":["decisions"],"operations":[{"type":"project","fields":[{"input_id":"decisions","field_id":"status"}]},{"type":"filter","field":{"input_id":"decisions","field_id":"status"},"comparison":"eq","value":{"type":"enum","value":"reviewed"}}],"null_policy":"unknown"}}
  ],
  "record_schemas": [
    {"id":"decision","version":1,"fields":[{"id":"status","value_type":{"type":"enum","values":["needs_context","reviewed"]},"nullable":false},{"id":"note","value_type":{"type":"text"},"nullable":false}]},
    {"id":"observation","version":1,"fields":[{"id":"state","value_type":{"type":"enum","values":["open","noted"]},"nullable":false},{"id":"note","value_type":{"type":"text"},"nullable":true}]}
  ],
  "commands": [
    {"id":"record_decision","record_schema_id":"decision","operation":"create","writable_fields":["status","note"],"transitions":[{"field_id":"status","from":"needs_context","to":"reviewed"}]},
    {"id":"annotate","record_schema_id":"observation","operation":"update","writable_fields":["state","note"],"transitions":[{"field_id":"state","from":"open","to":"noted"}]}
  ],
  "links": [
    {"from_view":"total","to_view":"destinations","input_id":"events"}
  ],
  "default_layout": [
    {"view_id":"total","column":0,"row":0,"width":6},
    {"view_id":"destinations","column":6,"row":0,"width":6},
    {"view_id":"timeline","column":0,"row":1,"width":12},
    {"view_id":"related","column":0,"row":2,"width":4},
    {"view_id":"spend","column":4,"row":2,"width":4},
    {"view_id":"regions","column":8,"row":2,"width":4},
    {"view_id":"record","column":0,"row":3,"width":6},
    {"view_id":"caution","column":6,"row":3,"width":6},
    {"view_id":"status","column":11,"row":100,"width":1}
  ]
}"#;
const RICH_DIGEST: &str = "sha256:2adf02a6f2bcd16942282fc159b0713fe0f7b9e3c81ff3c4dcc31f317ce50043";

/// One change to the rich definition (JSON pointer, replacement or `None` to
/// remove, content digest recomputed), the Rust code it must produce (`""` when
/// admitted) and validate.py's own message for its reference rules. validate.py
/// refused every coded row (its schema step for rows without a message) and
/// admitted every `""` row.
#[rustfmt::skip]
const RULE_CASES: [(&str, Option<&str>, &str, &str); 120] = [
    ("/content/inputs/1/fields/1/id", Some(r#""llllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllll""#), "", ""),
    ("/content/views/1/query/operations/5/value/coefficient", Some(r#""10000000000000000000000000000000000000""#), "", ""),
    ("/content/views/1/query/operations/5/value/coefficient", Some(r#""-12345""#), "", ""),
    ("/content/views/1/query/operations/4/value/value", Some(r#""2024-02-29T00:00:00Z""#), "", ""),
    ("/content/views/2/query/operations/2/denominator/value", Some(r#""1""#), "", ""),
    ("/content/views/0/query/operations/4/rows", Some(r#"1"#), "", ""),
    ("/content/inputs/0/fields/3/value_type/scale", Some(r#"18"#), "encoding", "filter value type/unit mismatch"),
    ("/content/inputs/1/fields/1/id", Some(r#""lllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllll""#), "encoding", ""),
    ("/revision_id", Some(r#""00000000-0000-4000-8000-00000000006A""#), "encoding", ""),
    ("/revision_id", Some(r#""00000000-0000-1000-8000-000000000067""#), "encoding", ""),
    ("/organization_id", Some(r#""00000000-0000-4000-c000-000000000064""#), "encoding", ""),
    ("/digest", Some(r#""sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA""#), "encoding", ""),
    ("/kind", Some(r#""publication""#), "encoding", ""),
    ("/schema_version", Some(r#"2"#), "encoding", ""),
    ("/content/views/1/query/operations/5/value/coefficient", Some(r#""100000000000000000000000000000000000000""#), "encoding", ""),
    ("/content/views/1/query/operations/5/value/scale", Some(r#"19"#), "encoding", ""),
    ("/content/inputs/0/fields/4/value_type/minor_unit_exponent", Some(r#"3"#), "encoding", ""),
    ("/content/inputs/0/fields/4/value_type/currency", Some(r#""usd""#), "encoding", ""),
    ("/content/views/1/query/operations/4/value/value", Some(r#""2026-02-29T00:00:00Z""#), "encoding", ""),
    ("/content/views/1/query/operations/4/value/value", Some(r#""2026-10-28T24:00:00Z""#), "encoding", ""),
    ("/content/views/1/query/operations/4/value/value", Some(r#""2026-10-28T12:00:60Z""#), "encoding", ""),
    ("/content/views/1/query/operations/4/value/value", Some(r#""2026-10-28 12:00:00Z""#), "encoding", ""),
    ("/content/views/1/query/operations/3/value/value", Some(r#""0000-01-01""#), "encoding", ""),
    ("/content/views/1/query/operations/3/value/value", Some(r#""2026-04-31""#), "encoding", ""),
    ("/content/views/0/query/window/start", Some(r#""2026-13-01T00:00:00Z""#), "encoding", ""),
    ("/content/default_layout/2/width", Some(r#"0"#), "encoding", ""),
    ("/content/default_layout/2/width", Some(r#"13"#), "encoding", ""),
    ("/content/default_layout/0/column", Some(r#"12"#), "encoding", ""),
    ("/content/default_layout/0/row", Some(r#"101"#), "encoding", ""),
    ("/content/default_layout/0/row", Some(r#"-1"#), "encoding", ""),
    ("/content/views/2/query/operations/0/seconds", Some(r#"7200"#), "encoding", ""),
    ("/content/views/2/query/operations/0/timezone", Some(r#""America/New_York""#), "encoding", ""),
    ("/content/inputs/0/fields/0/nullable", Some(r#""no""#), "encoding", ""),
    ("/content/inputs/0/fields/0/nullable", Some(r#"0"#), "encoding", ""),
    ("/content/inputs/0/fields/5/value_type/values", Some(r#"["east","east"]"#), "encoding", ""),
    ("/content/inputs/0/fields/5/value_type/values", Some(r#"[]"#), "encoding", ""),
    ("/content/views/1/query/inputs", Some(r#"["events","events"]"#), "encoding", ""),
    ("/content/views/1/query/inputs", Some(r#"["events","inventory","a","b","c","d","e","f","g"]"#), "encoding", ""),
    ("/content/commands/0/writable_fields", Some(r#"["status","status"]"#), "encoding", ""),
    ("/content/views/0/query/operations/3/fields", Some(r#"[{"field":{"input_id":"events","field_id":"region"},"direction":"asc"},{"field":{"input_id":"events","field_id":"region"},"direction":"asc"},{"field":{"input_id":"events","field_id":"region"},"direction":"asc"},{"field":{"input_id":"events","field_id":"region"},"direction":"asc"},{"field":{"input_id":"events","field_id":"region"},"direction":"asc"}]"#), "encoding", ""),
    ("/content/views/0/query/operations/1/group_by", Some(r#"[{"input_id":"events","field_id":"region"},{"input_id":"events","field_id":"region"},{"input_id":"events","field_id":"region"},{"input_id":"events","field_id":"region"},{"input_id":"events","field_id":"region"}]"#), "encoding", ""),
    ("/content/views/0/query/operations/1/measures", Some(r#"[{"id":"m0","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m1","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m2","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m3","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m4","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m5","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m6","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m7","operation":"count","field":{"input_id":"events","field_id":"bytes"}},{"id":"m8","operation":"count","field":{"input_id":"events","field_id":"bytes"}}]"#), "encoding", ""),
    ("/content/views/2/query/operations/2/denominator/value", Some(r#""0""#), "encoding", ""),
    ("/content/views/2/query/operations/2/denominator/value", Some(r#""1111111111111111111""#), "encoding", ""),
    ("/content/views/2/query/operations/2/denominator", Some(r#"{"type":"field","id":"rows"}"#), "encoding", ""),
    ("/content/views/3/query/operations/1/value/value", Some(r#""-0""#), "encoding", ""),
    ("/content/views/3/query/operations/0/value/record_id", Some(r#""not-a-uuid""#), "encoding", ""),
    ("/content/views/0/query/operations/0/value/value", Some(r#""true""#), "encoding", ""),
    ("/content/views/0/query/operations/0/value", Some(r#"{"type":"float","value":"1.5"}"#), "encoding", ""),
    ("/content/inputs/0/kind", Some(r#""stream""#), "encoding", ""),
    ("/content/inputs/0/contract_version", Some(r#"2"#), "encoding", ""),
    ("/content/record_schemas/0/version", Some(r#"2"#), "encoding", ""),
    ("/content/commands/0/operation", Some(r#""delete""#), "encoding", ""),
    ("/content/views/0/query/null_policy", Some(r#""zero""#), "encoding", ""),
    ("/content/views/0/query/window/bounds", Some(r#""closed""#), "encoding", ""),
    ("/content/views/1/query/operations/0/cardinality", Some(r#""many""#), "encoding", ""),
    ("/content/views/0/query/operations/2/zero_denominator", Some(r#""zero""#), "encoding", ""),
    ("/content/views/0/query/operations/2/rounding", Some(r#""half_even""#), "encoding", ""),
    ("/content/views/0/query/operations/3/fields/0/direction", Some(r#""up""#), "encoding", ""),
    ("/content/views/0/query/operations/4/rows", Some(r#"501"#), "encoding", ""),
    ("/content/views/0/query/operations/4/rows", Some(r#"0"#), "encoding", ""),
    ("/content/inputs/0/fields/2/value_type/unit", Some(r#""bytes/s""#), "encoding", ""),
    ("/content/views", Some(r#"[]"#), "encoding", ""),
    ("/content/views/0/query/operations", Some(r#"[]"#), "encoding", ""),
    ("/content/views/0/title", Some(r#"null"#), "encoding", ""),
    ("/content/description", None, "encoding", ""),
    ("/content/views/0/renderer_version", Some(r#"true"#), "unregistered_renderer", ""),
    ("/content/views/0/query/operator_version", Some(r#""1""#), "unregistered_operator", ""),
    ("/content/views/0/query/operations/0/comparison", Some(r#""like""#), "unregistered_operator", ""),
    ("/content/views/0/query/operations/1/measures/0/operation", Some(r#""avg""#), "unregistered_operator", ""),
    ("/content/views/1/query/operations/0/rule", Some(r#""fuzzy""#), "unregistered_operator", ""),
    ("/content/links/0/weight", Some(r#"1"#), "unknown_field", ""),
    ("/content/inputs/0/fields/0/value_type", Some(r#"{"type":"timestamp","timezone":"UTC"}"#), "unknown_field", ""),
    ("/content/inputs/1/id", Some(r#""events""#), "encoding", "duplicate input id"),
    ("/content/inputs/0/fields/1/id", Some(r#""time""#), "encoding", "duplicate input field"),
    ("/content/record_schemas/1/id", Some(r#""decision""#), "encoding", "duplicate record schema"),
    ("/content/record_schemas/0/fields/1/id", Some(r#""status""#), "encoding", "duplicate record field"),
    ("/content/views/1/id", Some(r#""total""#), "encoding", "duplicate view id"),
    ("/content/commands/1/id", Some(r#""record_decision""#), "encoding", "duplicate command id"),
    ("/content/inputs/0/fields/6/value_type/target_input", Some(r#""nowhere""#), "encoding", "unknown reference target"),
    ("/content/record_schemas/1/fields/0/value_type", Some(r#"{"type":"reference","target_input":"nowhere"}"#), "encoding", "unknown reference target"),
    ("/content/commands/0/record_schema_id", Some(r#""nowhere""#), "encoding", "unknown record schema"),
    ("/content/commands/0/writable_fields/1", Some(r#""secret""#), "encoding", "unknown writable field"),
    ("/content/commands/0/transitions/0/field_id", Some(r#""note""#), "encoding", "transition requires enum field"),
    ("/content/commands/0/writable_fields", Some(r#"["note"]"#), "encoding", "transition field is not writable"),
    ("/content/commands/0/transitions/0/to", Some(r#""approved""#), "encoding", "unknown transition value"),
    ("/content/views/0/command_id", Some(r#""record_decision""#), "encoding", "only a form invokes a command"),
    ("/content/views/7/text", None, "encoding", "note requires literal text"),
    ("/content/views/7/query", Some(r#"{"operator_version":1,"inputs":["events"],"operations":[{"type":"project","fields":[{"input_id":"events","field_id":"bytes"}]}],"null_policy":"unknown"}"#), "encoding", "note requires literal text"),
    ("/content/views/0/text", Some(r#""x""#), "encoding", "literal text only belongs to notes"),
    ("/content/views/0/query", None, "encoding", "data view requires query"),
    ("/content/views/6/query", Some(r#"{"operator_version":1,"inputs":["events"],"operations":[{"type":"project","fields":[{"input_id":"events","field_id":"bytes"}]}],"null_policy":"unknown"}"#), "encoding", "form requires declared command without query"),
    ("/content/views/6/command_id", None, "encoding", "form requires declared command without query"),
    ("/content/views/6/command_id", Some(r#""annotate_all""#), "encoding", "form requires declared command without query"),
    ("/content/views/0/query/window/end", Some(r#""2026-10-01T00:00:00Z""#), "encoding", "time window must be increasing"),
    ("/content/views/0/query/window/end", Some(r#""2026-09-30T23:59:59Z""#), "encoding", "time window must be increasing"),
    ("/content/views/1/query/inputs", Some(r#"["events","ghost"]"#), "encoding", "query names unknown input"),
    ("/content/views/0/query/operations/0/field/input_id", Some(r#""inventory""#), "encoding", "field uses undeclared query input"),
    ("/content/views/5/query/operations/0/fields/0/field_id", Some(r#""ghost""#), "encoding", "unknown query field"),
    ("/content/views/3/query/operations/1/value/unit", Some(r#""kib""#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/1/query/operations/5/value/scale", Some(r#"3"#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/1/query/operations/5/value/unit", Some(r#""eur""#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/1/query/operations/2/value/value", Some(r#""north""#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/3/query/operations/0/value/input_id", Some(r#""events""#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/1/query/operations/4/value", Some(r#"{"type":"null"}"#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/1/query/operations/7/value", Some(r#"{"type":"boolean","value":false}"#), "encoding", "filter value type/unit mismatch"),
    ("/content/views/0/query/operations/1/measures/1/id", Some(r#""total_bytes""#), "encoding", "duplicate measure id"),
    ("/content/views/0/query/operations/2/output_field", Some(r#""rows""#), "encoding", "duplicate derived measure"),
    ("/content/views/2/query/operations/0/field/field_id", Some(r#""day""#), "encoding", "time bucket requires timestamp"),
    ("/content/views/1/query/operations/0/right", Some(r#"{"input_id":"events","field_id":"bytes"}"#), "encoding", "lookup type mismatch"),
    ("/content/views/1/query/operations/0/left", Some(r#"{"input_id":"events","field_id":"bytes"}"#), "encoding", "lookup type mismatch"),
    ("/content/views/1/query/operations/0", Some(r#"{"type":"lookup","left":{"input_id":"events","field_id":"bytes"},"right":{"input_id":"events","field_id":"bytes"},"rule":"cidr_exact_then_longest","cardinality":"at_most_one"}"#), "encoding", "CIDR requires text fields"),
    ("/content/views/0/query/operations/2/numerator", Some(r#""nothing""#), "encoding", "ratio requires prior declared numerator"),
    ("/content/views/0/query/operations/2/denominator/id", Some(r#""nothing""#), "encoding", "ratio requires prior declared denominator"),
    ("/content/views/4/query/operations/0/measures/0/field/field_id", Some(r#""day""#), "encoding", "sum requires numeric field"),
    ("/content/default_layout/8/view_id", Some(r#""total""#), "encoding", "duplicate layout view"),
    ("/content/default_layout/8/view_id", Some(r#""ghost""#), "encoding", "layout must name each view exactly once"),
    ("/content/default_layout/1/width", Some(r#"7"#), "encoding", "layout exceeds grid"),
    ("/content/links/0/to_view", Some(r#""ghost""#), "encoding", "unknown link target"),
    ("/content/links/0/input_id", Some(r#""ghost""#), "encoding", "unknown link target"),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A locked fixture, after proving its embedded bytes are the ones the lock names.
fn locked(path: &str) -> Value {
    let lock: Value = serde_json::from_str(LOCK).unwrap();
    let (_, text) = FIXTURES
        .iter()
        .find(|(name, _)| *name == path)
        .unwrap_or_else(|| panic!("{path} is not embedded"));
    assert_eq!(
        lock["artifacts"][path],
        hex(&Sha256::digest(text.as_bytes())),
        "{path} differs from contracts/implementation-lock.json"
    );
    serde_json::from_str(text).unwrap()
}

fn bundle(name: &str) -> Value {
    locked(&format!("contracts/fixtures/{name}.json"))
}

fn northstar_definition() -> Value {
    bundle("northstar")["definition"].clone()
}

fn rich_definition() -> Value {
    let mut definition = json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": "00000000-0000-4000-8000-000000000064",
        "app_id": "00000000-0000-4000-8000-000000000065",
        "revision_id": "00000000-0000-4000-8000-000000000067",
        "content": serde_json::from_str::<Value>(RICH_CONTENT).unwrap(),
        "digest": "",
    });
    rehash(&mut definition);
    definition
}

fn admit(definition: &Value) -> Result<AdmittedDefinition, AdmissionError> {
    admit_definition(&serde_json::to_vec(definition).unwrap())
}

/// Keeps the submitted digest consistent so a test isolates another rule.
fn rehash(definition: &mut Value) {
    let canonical = encode(&definition["content"]).unwrap();
    definition["digest"] = json!(digest(digest::APP_DEFINITION, &canonical));
}

/// Replaces or inserts the member at a JSON pointer, or removes it for `None`.
fn set(document: &mut Value, pointer: &str, value: Option<Value>) {
    let (parent, last) = pointer.rsplit_once('/').unwrap();
    match (document.pointer_mut(parent), value) {
        (Some(Value::Object(map)), Some(value)) => {
            map.insert(last.to_owned(), value);
        }
        (Some(Value::Object(map)), None) => {
            map.remove(last).unwrap();
        }
        (Some(Value::Array(items)), Some(value)) => items[last.parse::<usize>().unwrap()] = value,
        _ => panic!("no member at {pointer}"),
    }
}

/// Applies one negative vector exactly as contracts/test_contracts.py does.
fn mutate(document: &mut Value, vector: &Value) {
    let path = vector["path"].as_array().unwrap();
    let (last, parents) = path.split_last().unwrap();
    let target = parents.iter().fold(document, |target, step| match step {
        Value::String(key) => &mut target[key.as_str()],
        step => &mut target[step.as_u64().unwrap() as usize],
    });
    match (last, vector.get("operation").and_then(Value::as_str)) {
        (Value::String(key), Some("remove")) => {
            target.as_object_mut().unwrap().remove(key);
        }
        (Value::String(key), _) => target[key.as_str()] = vector["value"].clone(),
        (index, _) => target[index.as_u64().unwrap() as usize] = vector["value"].clone(),
    }
}

/// Coded reasons for the negative vectors validate.py refuses at its schema step.
fn schema_vector(name: &str) -> (&'static str, &'static str) {
    match name {
        "model code is refused" => ("executable_content", "$.content.javascript"),
        "model grants are refused" => ("client_grant", "$.content.grants"),
        "model SQL is refused" => ("executable_content", "$.content.views[0].query.sql"),
        "unknown renderer refused" => ("unregistered_renderer", "$.content.views[0].kind"),
        "unknown operator refused" => (
            "unregistered_operator",
            "$.content.views[0].query.operations[0].type",
        ),
        // Edition one's vector sets schema version 2 on an edition-one document.
        // Edition two registers version 2, so the refusal now names the first
        // edition-two member the document lacks; the verdict is unchanged.
        "unknown version refused" => ("encoding", "$.content.inputs[0].coverage"),
        "unregistered schema version refused" => ("encoding", "$.schema_version"),
        "edition two input without coverage refused" | "unknown coverage refused" => {
            ("encoding", "$.content.inputs[0].coverage")
        }
        "client supplied window length refused" => (
            "unknown_field",
            "$.content.views[5].query.operations[1].denominator.minutes",
        ),
        "nonopaque entity identifier refused" => ("encoding", "$.app_id"),
        "concrete source in reusable input refused" => {
            ("unknown_field", "$.content.inputs[0].source_revision_id")
        }
        "unsafe exact money representation refused" => (
            "encoding",
            "$.content.views[1].query.operations[0].value.minor_units",
        ),
        "unsupported currency refused" => (
            "encoding",
            "$.content.views[1].query.operations[0].value.currency",
        ),
        "missing layout refused" => ("encoding", "$.content.default_layout"),
        "trailing newline identifier refused" => ("encoding", "$.content.views[0].id"),
        other => panic!("unclassified schema vector: {other}"),
    }
}

fn nested_arrays(depth: usize, inner: &str) -> Vec<u8> {
    format!("{}{inner}{}", "[".repeat(depth), "]".repeat(depth)).into_bytes()
}

/// The Northstar definition with `count` independent note views.
fn with_note_views(count: usize) -> Value {
    let mut definition = northstar_definition();
    definition["content"]["views"] = (0..count)
        .map(|i| json!({"id": format!("note_{i}"), "kind": "note", "renderer_version": 1, "title": "", "text": ""}))
        .collect();
    definition["content"]["default_layout"] = (0..count)
        .map(|i| json!({"view_id": format!("note_{i}"), "column": 0, "row": i, "width": 12}))
        .collect();
    rehash(&mut definition);
    definition
}

/// Twenty-four note views whose free text brings the canonical content to exactly
/// `target` bytes; four-byte characters show the bound counts bytes.
fn sized_definition(target: usize) -> Value {
    let mut definition = with_note_views(24);
    let mut remaining = target - encode(&definition["content"]).unwrap().len();
    for view in definition["content"]["views"].as_array_mut().unwrap() {
        for key in ["title", "text"] {
            let wide = (remaining / 4).min(2000);
            let narrow = (remaining - wide * 4).min(2000 - wide);
            remaining -= wide * 4 + narrow;
            view[key] = json!(format!(
                "{}{}",
                "\u{1f600}".repeat(wide),
                "y".repeat(narrow)
            ));
        }
    }
    assert_eq!(remaining, 0);
    rehash(&mut definition);
    assert_eq!(encode(&definition["content"]).unwrap().len(), target);
    definition
}

#[test]
fn canonical_vectors_match_contracts_lock() {
    // The embedded vectors and specimens are exactly the locked edition's set.
    let lock: Value = serde_json::from_str(LOCK).unwrap();
    assert_eq!(lock["edition"], "impl-2026-10-29.3");
    assert_eq!(lock["canonical_encoding"], "forma-json-v1");
    let locked_fixtures: BTreeSet<&str> = lock["artifacts"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|path| path.starts_with("contracts/fixtures/"))
        .collect();
    assert_eq!(
        locked_fixtures,
        FIXTURES.iter().map(|(path, _)| *path).collect()
    );

    // Positive vectors: identical bytes and digests from the value and from text.
    let vectors = locked("contracts/fixtures/canonical-vectors.json");
    assert_eq!(vectors.as_array().unwrap().len(), EDITION_TWO_VECTORS);
    for vector in vectors.as_array().unwrap() {
        let name = vector["name"].as_str().unwrap();
        let domain = vector
            .get("domain")
            .and_then(Value::as_str)
            .unwrap_or(digest::RESULT);
        let canonical = encode(&vector["value"]).unwrap();
        assert_eq!(hex(&canonical), vector["utf8_hex"], "{name}");
        assert_eq!(digest(domain, &canonical), vector["digest"], "{name}");
        let spaced = serde_json::to_vec_pretty(&vector["value"]).unwrap();
        assert_eq!(
            encode(&decode(&spaced).unwrap()).unwrap(),
            canonical,
            "{name}"
        );
    }

    // Negative vectors: each definition mutation fails whole-output admission
    // with its coded reason; validate.py's own message for its reference rules.
    assert_eq!(
        negative_vectors("contracts/fixtures/negative-vectors.json", bundle),
        EDITION_TWO_DEFINITION_VECTORS
    );
    canonical_refusals();
}

/// Applies every negative vector of one locked file to the bundles `load`
/// returns; returns how many mutate a definition, each of which admission
/// refuses with its coded reason.
fn negative_vectors(path: &str, load: fn(&str) -> Value) -> usize {
    let mut definition_vectors = 0;
    for vector in locked(path).as_array().unwrap() {
        let name = vector["name"].as_str().unwrap();
        let fixture = vector
            .get("fixture")
            .and_then(Value::as_str)
            .unwrap_or("northstar");
        let mut document = load(fixture);
        let before = document["definition"].clone();
        mutate(&mut document, vector);
        let entity = vector["path"][0].as_str().unwrap();
        if entity != "definition" {
            // Publication, saved work, source, receipt, command and outcome
            // contracts belong to the persistence, query and command slices;
            // these vectors leave the definition, and so its admission, unchanged.
            assert!(
                [
                    "publication",
                    "work",
                    "sources",
                    "receipt",
                    "command",
                    "outcome"
                ]
                .contains(&entity),
                "{name}"
            );
            assert_eq!(document["definition"], before, "{name}");
            continue;
        }
        definition_vectors += 1;
        if vector["rehash_definition"] == true {
            rehash(&mut document["definition"]);
        }
        let error = admit(&document["definition"]).expect_err(name);
        let expected = vector["expected"].as_str().unwrap();
        let (code, detail) = match expected {
            "schema:" => schema_vector(name),
            message => ("encoding", message),
        };
        assert_eq!(error.code(), code, "{name}: {error}");
        assert!(error.to_string().contains(detail), "{name}: {error}");
    }
    definition_vectors
}

fn canonical_refusals() {
    // validate.py canonical(): key order never matters, array order, Unicode
    // form and domain always do, and unrepresentable values are refused.
    let reordered = decode(br#"{"b":2,"a":1}"#).unwrap();
    assert_eq!(encode(&reordered).unwrap(), br#"{"a":1,"b":2}"#);
    assert_ne!(
        digest(digest::RESULT, &encode(&json!([1, 2])).unwrap()),
        digest(digest::RESULT, &encode(&json!([2, 1])).unwrap())
    );
    assert_ne!(
        encode(&json!("\u{e9}")).unwrap(),
        encode(&json!("e\u{301}")).unwrap()
    );
    for value in [
        json!({"a": 0.1}),
        json!(1.0),
        json!(9_007_199_254_740_992_u64),
        json!(-9_007_199_254_740_992_i64),
        json!({"A": 1}),
        json!({"\u{e9}": 1}),
        json!({"": 1}),
        json!({"a b": 1}),
        serde_json::from_slice(&nested_arrays(33, "null")).unwrap(),
    ] {
        assert!(matches!(encode(&value), Err(Encoding(_))), "{value}");
    }
    assert_eq!(
        encode(&json!(9_007_199_254_740_991_u64)).unwrap(),
        b"9007199254740991"
    );
    assert_eq!(
        encode(&json!(-9_007_199_254_740_991_i64)).unwrap(),
        b"-9007199254740991"
    );
    let fits = json!("a".repeat(MAX_DOCUMENT_BYTES - 2));
    assert_eq!(encode(&fits).unwrap().len(), MAX_DOCUMENT_BYTES);
    let over = json!("a".repeat(MAX_DOCUMENT_BYTES - 1));
    assert_eq!(encode(&over), Err(TooLarge(MAX_DOCUMENT_BYTES + 1)));
}

#[test]
fn strict_decoder_matches_python_loads() {
    for (raw, canonical, expected) in PYTHON_DECODED {
        let value = decode(raw).unwrap_or_else(|error| panic!("{raw:?}: {error}"));
        assert_eq!(encode(&value).unwrap(), canonical, "{raw:?}");
        assert_eq!(digest(digest::RESULT, canonical), expected, "{raw:?}");
    }
    // Depth counts value positions from the root at 0, so 32 is the deepest.
    for (raw, expected) in [
        (
            nested_arrays(32, "null"),
            "sha256:c00101cea0566df659b1d21f6bd5e8033b142a4c99a032223ce9f86da1db3849",
        ),
        (
            nested_arrays(33, ""),
            "sha256:59303b90fcd04006ed35d50a0477cf85f798e8052a1f16652781423724d79728",
        ),
    ] {
        let canonical = encode(&decode(&raw).unwrap()).unwrap();
        assert_eq!(canonical, raw);
        assert_eq!(digest(digest::RESULT, &canonical), expected);
    }
    for raw in PYTHON_REFUSED {
        assert!(
            matches!(decode(raw), Err(Encoding(_))),
            "{raw:?} was not refused"
        );
    }
    for raw in [
        nested_arrays(33, "null"),
        nested_arrays(34, ""),
        nested_arrays(2000, "null"),
    ] {
        assert!(
            matches!(decode(&raw), Err(Encoding(_))),
            "depth {}",
            raw.len()
        );
    }
    // The 1 MiB bound applies to the raw bytes before any parsing.
    let mut padded = b"{}".to_vec();
    padded.resize(MAX_DOCUMENT_BYTES, b' ');
    assert_eq!(decode(&padded).unwrap(), json!({}));
    padded.push(b' ');
    assert_eq!(decode(&padded), Err(TooLarge(MAX_DOCUMENT_BYTES + 1)));
}

#[test]
fn digests_are_domain_separated_and_registered() {
    assert_eq!(DOMAINS, DOMAIN_DIGESTS.map(|(domain, _)| domain));
    for (domain, expected) in DOMAIN_DIGESTS {
        assert_eq!(digest(domain, b"{}"), expected, "{domain}");
    }
    let identity = digest(digest::QUERY, b"{}");
    assert_eq!(identity.len(), 71);
    assert!(identity["sha256:".len()..]
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
}

#[test]
#[should_panic(expected = "unregistered digest domain")]
fn unregistered_digest_domain_is_refused() {
    digest("forma:arbitrary:v1", b"{}");
}

#[test]
fn fixtures_digests_match_python() {
    let mut definitions = BTreeSet::new();
    for name in ["northstar", "spend-review"] {
        let path = format!("contracts/fixtures/{name}.json");
        let bundle = locked(&path);
        // The strict decoder reads each whole specimen exactly as serde_json does.
        let (_, text) = FIXTURES
            .iter()
            .find(|(fixture, _)| *fixture == path)
            .unwrap();
        assert_eq!(decode(text.as_bytes()).unwrap(), bundle, "{name}");
        let of = |domain: &str, value: &Value| digest(domain, &encode(value).unwrap());

        let definition = &bundle["definition"];
        let content = &definition["content"];
        let admitted = admit(definition).unwrap();
        assert_eq!(definition["digest"], admitted.digest, "{name}");
        assert_eq!(admitted.canonical, encode(content).unwrap(), "{name}");
        assert_eq!(
            digest(digest::APP_DEFINITION, &admitted.canonical),
            admitted.digest
        );
        assert_eq!(
            (
                admitted.schema_version,
                admitted.renderer_version,
                admitted.operator_version
            ),
            (2, 1, 1)
        );
        assert_eq!(
            admitted.view_count,
            content["views"].as_array().unwrap().len()
        );
        definitions.insert(admitted.digest.clone());

        // Publication: definition, input contract and record schema identities.
        let publication = &bundle["publication"];
        assert_eq!(publication["definition_digest"], admitted.digest, "{name}");
        let inputs = content["inputs"].as_array().unwrap();
        let contracts: Vec<Value> = inputs
            .iter()
            .map(|input| json!({"input_id": input["id"], "digest": of(digest::INPUT_CONTRACT, input)}))
            .collect();
        assert_eq!(
            publication["input_contract_digests"],
            json!(contracts),
            "{name}"
        );
        let schemas: Vec<Value> = content["record_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|schema| json!({"schema_id": schema["id"], "digest": of(digest::RECORD_SCHEMA, schema)}))
            .collect();
        assert_eq!(
            publication["record_schema_digests"],
            json!(schemas),
            "{name}"
        );

        // Saved work binds each input by its contract identity.
        for binding in bundle["work"]["bindings"].as_array().unwrap() {
            let input = inputs
                .iter()
                .find(|input| input["id"] == binding["input_id"])
                .unwrap();
            assert_eq!(
                binding["contract_digest"],
                of(digest::INPUT_CONTRACT, input),
                "{name}"
            );
        }

        // Receipt: definition, query, result and its own identity.
        let receipt = &bundle["receipt"];
        assert_eq!(receipt["definition_digest"], admitted.digest, "{name}");
        let view = content["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["id"] == receipt["view_id"])
            .unwrap();
        assert_eq!(
            receipt["provenance"]["query_digest"],
            of(digest::QUERY, &view["query"])
        );
        assert_eq!(
            receipt["provenance"]["predicate_digest"],
            of(digest::QUERY, &view["query"])
        );
        assert_eq!(
            receipt["result_digest"],
            of(digest::RESULT, &json!({"example": content["title"]})),
            "{name}"
        );
        let mut unsigned = receipt.clone();
        unsigned.as_object_mut().unwrap().remove("digest");
        assert_eq!(
            receipt["digest"],
            of(digest::QUERY_RECEIPT, &unsigned),
            "{name}"
        );

        // Command payload identity excludes only the retry key.
        let mut payload = bundle["command"].clone();
        payload.as_object_mut().unwrap().remove("idempotency_key");
        assert_eq!(
            bundle["outcome"]["payload_digest"],
            of(digest::COMMAND_PAYLOAD, &payload),
            "{name}"
        );
    }
    assert_eq!(definitions.len(), 2);
}

#[test]
fn rejects_unknown_fields_executable_strings_and_client_grants_with_coded_reason() {
    let uuid = json!("00000000-0000-4000-8000-000000000555");
    let cases = [
        // Outside the closed grammar.
        (
            "/content/theme",
            json!("dark"),
            UnknownField("$.content.theme".into()),
        ),
        (
            "/content/views/0/query/cache_seconds",
            json!(60),
            UnknownField("$.content.views[0].query.cache_seconds".into()),
        ),
        (
            "/content/inputs/0/source_revision_id",
            uuid.clone(),
            UnknownField("$.content.inputs[0].source_revision_id".into()),
        ),
        // Executable shapes where the contract declares no free text.
        (
            "/content/views/0/id",
            json!("<script>alert(1)</script>"),
            ExecutableContent("$.content.views[0].id".into()),
        ),
        (
            "/content/views/0/kind",
            json!("<iframe src=x>"),
            ExecutableContent("$.content.views[0].kind".into()),
        ),
        (
            "/content/inputs/0/fields/2/value_type/unit",
            json!("https://evil.example/exfiltrate"),
            ExecutableContent("$.content.inputs[0].fields[2].value_type.unit".into()),
        ),
        (
            "/content/record_schemas/0/fields/0/value_type/values/1",
            json!("../../etc/passwd"),
            ExecutableContent("$.content.record_schemas[0].fields[0].value_type.values[1]".into()),
        ),
        (
            "/content/views/0/query/operations/0/measures/0/id",
            json!("total; DROP TABLE records"),
            ExecutableContent("$.content.views[0].query.operations[0].measures[0].id".into()),
        ),
        (
            "/content/commands/0/id",
            json!("javascript:alert(document.cookie)"),
            ExecutableContent("$.content.commands[0].id".into()),
        ),
        (
            "/content/views/1/query/operations/0/rule",
            json!("exact' OR '1'='1' --"),
            ExecutableContent("$.content.views[1].query.operations[0].rule".into()),
        ),
        (
            "/content/views/2/query/operations/0/timezone",
            json!("${process.env.SECRET}"),
            ExecutableContent("$.content.views[2].query.operations[0].timezone".into()),
        ),
        (
            "/content/default_layout/0/view_id",
            json!("SELECT * FROM secrets"),
            ExecutableContent("$.content.default_layout[0].view_id".into()),
        ),
        (
            "/app_id",
            json!("fetch(secret)"),
            ExecutableContent("$.app_id".into()),
        ),
        // Values of the wrong JSON type are searched like undeclared content.
        (
            "/content/views/0/title",
            json!({"html": "<b>x</b>"}),
            ExecutableContent("$.content.views[0].title.html".into()),
        ),
        (
            "/content/default_layout/0/column",
            json!("<script>alert(1)</script>"),
            ExecutableContent("$.content.default_layout[0].column".into()),
        ),
        (
            "/content/views/0/query/operations/0/measures",
            json!("SELECT * FROM secrets"),
            ExecutableContent("$.content.views[0].query.operations[0].measures".into()),
        ),
        (
            "/content/views/0/renderer_version",
            json!("javascript:alert(1)"),
            ExecutableContent("$.content.views[0].renderer_version".into()),
        ),
        // Undeclared keys naming or carrying executable content.
        (
            "/content/javascript",
            json!("return fetch(secret)"),
            ExecutableContent("$.content.javascript".into()),
        ),
        (
            "/content/views/0/query/sql",
            json!("SELECT * FROM secrets"),
            ExecutableContent("$.content.views[0].query.sql".into()),
        ),
        (
            "/content/views/0/html",
            json!("<b>x</b>"),
            ExecutableContent("$.content.views[0].html".into()),
        ),
        (
            "/content/notes",
            json!("DELETE FROM records"),
            ExecutableContent("$.content.notes".into()),
        ),
        (
            "/content/views/0/query/operations/0",
            json!({"type": "sql", "sql": "SELECT * FROM secrets"}),
            ExecutableContent("$.content.views[0].query.operations[0].sql".into()),
        ),
        // Client-supplied authority.
        (
            "/content/grants",
            json!(["admin"]),
            ClientGrant("$.content.grants".into()),
        ),
        (
            "/content/views/0/permissions",
            json!(["record_write"]),
            ClientGrant("$.content.views[0].permissions".into()),
        ),
        ("/role", json!("owner"), ClientGrant("$.role".into())),
        (
            "/content/inputs/0/owner_principal_id",
            uuid.clone(),
            ClientGrant("$.content.inputs[0].owner_principal_id".into()),
        ),
        (
            "/content/membership_epoch",
            json!(7),
            ClientGrant("$.content.membership_epoch".into()),
        ),
        (
            "/content/organization_id",
            uuid,
            ClientGrant("$.content.organization_id".into()),
        ),
        (
            "/content/views/0/query/access_scope",
            json!("all"),
            ClientGrant("$.content.views[0].query.access_scope".into()),
        ),
        (
            "/content/settings",
            json!({"display": {"roles": ["admin"]}}),
            ClientGrant("$.content.settings.display.roles".into()),
        ),
        (
            "/content/inputs/0/fields/0/nullable",
            json!({"roles": ["admin"]}),
            ClientGrant("$.content.inputs[0].fields[0].nullable.roles".into()),
        ),
    ];
    for (pointer, value, expected) in cases {
        let mut definition = northstar_definition();
        set(&mut definition, pointer, Some(value));
        rehash(&mut definition);
        assert_eq!(admit(&definition), Err(expected), "{pointer}");
    }

    // Free text is data: markup, SQL, shell and URL syntax are admitted verbatim.
    let hostile = "<script>alert(1)</script>; SELECT * FROM secrets; $(touch /tmp/pwned) https://evil.example/../x";
    let mut definition = northstar_definition();
    for pointer in [
        "/content/title",
        "/content/description",
        "/content/views/0/title",
        "/content/views/4/text",
    ] {
        set(&mut definition, pointer, Some(json!(hostile)));
    }
    definition["content"]["views"][1]["query"]["operations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "type": "filter",
            "field": {"input_id": "events", "field_id": "destination"},
            "comparison": "ne",
            "value": {"type": "text", "value": hostile}
        }));
    rehash(&mut definition);
    let admitted = admit(&definition).unwrap();
    let escaped = serde_json::to_string(hostile).unwrap();
    assert_eq!(
        String::from_utf8(admitted.canonical)
            .unwrap()
            .matches(&escaped)
            .count(),
        5
    );

    // Several violations report the most severe class, wherever each occurs.
    let mut definition = northstar_definition();
    set(
        &mut definition,
        "/content/default_layout/0/width",
        Some(json!(13)),
    );
    set(&mut definition, "/content/theme", Some(json!("dark")));
    set(&mut definition, "/content/views/0/id", Some(json!("<b>")));
    set(&mut definition, "/content/grants", Some(json!(["admin"])));
    rehash(&mut definition);
    assert_eq!(
        admit(&definition),
        Err(ClientGrant("$.content.grants".into()))
    );
    set(&mut definition, "/content/grants", None);
    rehash(&mut definition);
    assert_eq!(
        admit(&definition),
        Err(ExecutableContent("$.content.views[0].id".into()))
    );
    set(&mut definition, "/content/views/0/id", Some(json!("total")));
    rehash(&mut definition);
    assert_eq!(
        admit(&definition),
        Err(UnknownField("$.content.theme".into()))
    );
    set(&mut definition, "/content/theme", None);
    rehash(&mut definition);
    let error = admit(&definition).unwrap_err();
    assert_eq!(error.code(), "encoding");
    assert!(
        error
            .to_string()
            .contains("$.content.default_layout[0].width"),
        "{error}"
    );

    // Codes are stable snake_case and lead the rendered reason.
    let codes = [
        (UnknownField("$.x".into()), "unknown_field"),
        (ExecutableContent("$.x".into()), "executable_content"),
        (TooManyViews(25), "too_many_views"),
        (TooManyOperations(9), "too_many_operations"),
        (TooLarge(262_145), "too_large"),
        (UnregisteredRenderer("$.x".into()), "unregistered_renderer"),
        (UnregisteredOperator("$.x".into()), "unregistered_operator"),
        (ClientGrant("$.x".into()), "client_grant"),
        (Encoding("$.x: rule".into()), "encoding"),
    ];
    for (error, code) in codes {
        assert_eq!(error.code(), code);
        assert!(
            error.to_string().starts_with(&format!("{code}: ")),
            "{error}"
        );
    }
}

#[test]
fn rejects_over_complex_definitions() {
    assert_eq!(admit(&with_note_views(24)).unwrap().view_count, 24);
    assert_eq!(admit(&with_note_views(25)), Err(TooManyViews(25)));

    let with_operations = |count: usize| {
        let mut definition = northstar_definition();
        let project =
            json!({"type": "project", "fields": [{"input_id": "events", "field_id": "bytes"}]});
        definition["content"]["views"][0]["query"]["operations"] = json!(vec![project; count]);
        rehash(&mut definition);
        definition
    };
    assert!(admit(&with_operations(8)).is_ok());
    assert_eq!(admit(&with_operations(9)), Err(TooManyOperations(9)));

    let fits = sized_definition(MAX_DEFINITION_BYTES);
    assert_eq!(admit(&fits).unwrap().canonical.len(), MAX_DEFINITION_BYTES);
    assert_eq!(
        admit(&sized_definition(MAX_DEFINITION_BYTES + 1)),
        Err(TooLarge(262_145))
    );

    // A page holds at most 500 rows.
    let with_limit = |rows: u64| {
        let mut definition = northstar_definition();
        definition["content"]["views"][1]["query"]["operations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "limit", "rows": rows}));
        rehash(&mut definition);
        definition
    };
    assert!(admit(&with_limit(500)).is_ok());
    let error = admit(&with_limit(501)).unwrap_err();
    assert_eq!(error.code(), "encoding");
    assert!(
        error
            .to_string()
            .contains("$.content.views[1].query.operations[2].rows"),
        "{error}"
    );

    // The 1 MiB document bound is applied to the raw bytes before parsing.
    let mut raw = serde_json::to_vec(&northstar_definition()).unwrap();
    raw.resize(MAX_DOCUMENT_BYTES, b' ');
    assert!(admit_definition(&raw).is_ok());
    raw.push(b' ');
    assert_eq!(
        admit_definition(&raw),
        Err(TooLarge(MAX_DOCUMENT_BYTES + 1))
    );
}

#[test]
fn unregistered_renderer_or_operator_version_refuses() {
    let cases = [
        (
            "/content/views/0/renderer_version",
            json!(2),
            UnregisteredRenderer("$.content.views[0].renderer_version".into()),
        ),
        (
            "/content/views/0/renderer_version",
            json!(0),
            UnregisteredRenderer("$.content.views[0].renderer_version".into()),
        ),
        (
            "/content/views/0/kind",
            json!("script"),
            UnregisteredRenderer("$.content.views[0].kind".into()),
        ),
        (
            "/content/views/0/kind",
            json!("Table"),
            UnregisteredRenderer("$.content.views[0].kind".into()),
        ),
        (
            "/content/views/0/query/operator_version",
            json!(2),
            UnregisteredOperator("$.content.views[0].query.operator_version".into()),
        ),
        (
            "/content/views/0/query/operations/4/type",
            json!("sql"),
            UnregisteredOperator("$.content.views[0].query.operations[4].type".into()),
        ),
        (
            "/content/views/0/query/operations/4/type",
            json!("join"),
            UnregisteredOperator("$.content.views[0].query.operations[4].type".into()),
        ),
        (
            "/content/views/0/query/operations/1/measures/0/operation",
            json!("median"),
            UnregisteredOperator(
                "$.content.views[0].query.operations[1].measures[0].operation".into(),
            ),
        ),
        (
            "/content/views/0/query/operations/0/comparison",
            json!("like"),
            UnregisteredOperator("$.content.views[0].query.operations[0].comparison".into()),
        ),
        (
            "/content/views/1/query/operations/0/rule",
            json!("regex"),
            UnregisteredOperator("$.content.views[1].query.operations[0].rule".into()),
        ),
    ];
    for (pointer, value, expected) in cases {
        let mut definition = rich_definition();
        set(&mut definition, pointer, Some(value));
        rehash(&mut definition);
        assert_eq!(admit(&definition), Err(expected), "{pointer}");
    }

    // Registered kinds and versions admit, and the identity names them.
    let admitted = admit(&rich_definition()).unwrap();
    assert_eq!(admitted.digest, RICH_DIGEST);
    assert_eq!(
        (
            admitted.schema_version,
            admitted.renderer_version,
            admitted.operator_version,
            admitted.view_count
        ),
        (1, 1, 1, 9)
    );
    for kind in [
        "summary", "table", "detail", "timeline", "chart", "filter", "status",
    ] {
        let mut definition = northstar_definition();
        definition["content"]["views"][0]["kind"] = json!(kind);
        rehash(&mut definition);
        assert!(admit(&definition).is_ok(), "{kind}");
    }
    // A schema version outside the registered editions is not an encoding of
    // either, and the edition-one shape is not an edition-two definition.
    let mut definition = northstar_definition();
    definition["schema_version"] = json!(3);
    assert_eq!(admit(&definition).unwrap_err().code(), "encoding");
    let mut definition = rich_definition();
    definition["schema_version"] = json!(2);
    assert_eq!(admit(&definition).unwrap_err().code(), "encoding");
}

#[test]
fn shape_and_reference_rules_match_validate_py() {
    let base = rich_definition();
    assert_eq!(admit(&base).unwrap().digest, RICH_DIGEST);
    for (pointer, value, code, rule) in RULE_CASES {
        let mut definition = base.clone();
        set(
            &mut definition,
            pointer,
            value.map(|text| serde_json::from_str(text).unwrap()),
        );
        if pointer.starts_with("/content") {
            rehash(&mut definition);
        }
        match (admit(&definition), code) {
            (Ok(_), "") => {}
            (Ok(_), code) => panic!("{pointer}: admitted, validate.py refused it ({code})"),
            (Err(error), "") => panic!("{pointer}: {error}, validate.py admitted it"),
            (Err(error), code) => {
                assert_eq!(error.code(), code, "{pointer}: {error}");
                assert!(error.to_string().contains(rule), "{pointer}: {error}");
            }
        }
    }
    // Free text is bounded in Unicode scalars, as validate.py's maxLength counts.
    for (pointer, text, admitted) in [
        ("/content/title", "\u{1f600}".repeat(2000), true),
        ("/content/title", "\u{1f600}".repeat(2001), false),
        ("/content/views/7/text", "x".repeat(2001), false),
    ] {
        let mut definition = base.clone();
        set(&mut definition, pointer, Some(json!(text)));
        rehash(&mut definition);
        match admit(&definition) {
            Ok(_) => assert!(admitted, "{pointer}"),
            Err(error) => assert!(
                !admitted && error.code() == "encoding",
                "{pointer}: {error}"
            ),
        }
    }
}

/// Canonical vectors of edition two: edition one's six, two normalized-row
/// streams, one job payload and four exact rationals.
const EDITION_TWO_VECTORS: usize = 13;
/// Edition two's negative vectors that mutate a definition.
const EDITION_TWO_DEFINITION_VECTORS: usize = 25;

/// Edition impl-2026-10-28.1's locked fixture identities; edition two retains
/// these copies byte-identically and the lock names them again.
const EDITION_ONE: [(&str, &str); 4] = [
    (
        "contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json",
        "33b46ff5970bdcd5259f23bdb5e9a7fcda4c5eeffebe93e0fedfebf5f661203d",
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/negative-vectors.json",
        "089e9a3017adda324a2a150fb68d5aa497889cd104d2f7096b9128d520764e75",
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/northstar.json",
        "45639ea369193d60f45fbafbe6c3aad648881017df57615bdf89bd501dfdd592",
    ),
    (
        "contracts/fixtures/impl-2026-10-28.1/spend-review.json",
        "4a4581f5a7fcfec867070584a00b0fbebdf33558fffe538eec35f97f72aa5650",
    ),
];

fn retained(name: &str) -> Value {
    locked(&format!("contracts/fixtures/impl-2026-10-28.1/{name}.json"))
}

#[test]
fn edition_two_vectors_match_contracts_lock() {
    let lock: Value = serde_json::from_str(LOCK).unwrap();
    assert_eq!(lock["edition"], "impl-2026-10-29.3");
    let retained_lock = &lock["retained_editions"]["impl-2026-10-28.1"]["artifacts"];
    for (path, identity) in EDITION_ONE {
        assert_eq!(retained_lock[path], identity, "{path}");
    }
    // Edition three (APP-013) retains edition two byte for byte: the locked
    // identities equal the embedded copies, which equal the current bundles.
    let retained_two = lock["retained_editions"]["impl-2026-10-29.2"]["artifacts"]
        .as_object()
        .unwrap();
    assert_eq!(retained_two.len(), 4);
    for (path, identity) in retained_two {
        let embedded = FIXTURES
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, text)| *text)
            .unwrap_or_else(|| panic!("{path} embedded"));
        let mut hasher = sha2::Sha256::new();
        hasher.update(embedded.as_bytes());
        assert_eq!(
            hex(&hasher.finalize()),
            identity.as_str().unwrap(),
            "{path}"
        );
        let current = path.replace("impl-2026-10-29.2/", "");
        assert_eq!(locked(&current), locked(path), "{path}");
    }

    // Every vector reproduces in its own registered domain; new domains appear.
    let vectors = locked("contracts/fixtures/canonical-vectors.json");
    let mut domains = BTreeSet::new();
    let mut rationals = 0;
    for vector in vectors.as_array().unwrap() {
        let name = vector["name"].as_str().unwrap();
        let domain = vector
            .get("domain")
            .and_then(Value::as_str)
            .unwrap_or(digest::RESULT);
        assert!(DOMAINS.contains(&domain), "{name}");
        let canonical = encode(&vector["value"]).unwrap();
        assert_eq!(hex(&canonical), vector["utf8_hex"], "{name}");
        assert_eq!(digest(domain, &canonical), vector["digest"], "{name}");
        domains.insert(domain);
        if let Some(rational) = vector.get("rational") {
            rationals += 1;
            let reduced = exact_rational(
                rational["numerator"].as_str().unwrap(),
                rational["denominator"].as_str().unwrap(),
            )
            .unwrap();
            assert_eq!(reduced, vector["value"], "{name}");
            check_exact_rational(&vector["value"]).unwrap();
        }
        if domain == digest::NORMALIZED_ROWS {
            // The streamed row identity equals the identity of the whole array.
            let rows = vector["value"].as_array().unwrap();
            assert_eq!(
                digest::normalized_rows(rows.iter()).unwrap(),
                vector["digest"],
                "{name}"
            );
        }
    }
    assert!(domains.contains(digest::NORMALIZED_ROWS));
    assert!(domains.contains(digest::JOB_PAYLOAD));
    assert!(rationals >= 3);
    assert_eq!(DOMAINS.len(), 10);

    // exact_rational: lowest terms, positive denominator, at most 38 digits.
    let largest = "9".repeat(38);
    assert!(exact_rational(&largest, "1").is_ok());
    let over = format!("1{}", "0".repeat(38));
    let under = format!("-1{}", "0".repeat(38));
    for (numerator, denominator, rule) in [
        ("1", "0", "zero denominator"),
        (over.as_str(), "1", "38 digits"),
        ("1", under.as_str(), "38 digits"),
        ("1.5", "1", "decimal integer"),
        ("01", "1", "decimal integer"),
        ("-0", "1", "decimal integer"),
        ("", "1", "decimal integer"),
    ] {
        let error = exact_rational(numerator, denominator).unwrap_err();
        assert!(
            error.to_string().contains(rule),
            "{numerator}/{denominator}: {error}"
        );
    }
    for (value, rule) in [
        (
            json!({"numerator": "2", "denominator": "4"}),
            "lowest terms",
        ),
        (
            json!({"numerator": "0", "denominator": "2"}),
            "lowest terms",
        ),
        (
            json!({"numerator": "-6", "denominator": "9"}),
            "lowest terms",
        ),
        (json!({"numerator": "1", "denominator": "0"}), "positive"),
        (json!({"numerator": "1", "denominator": "-2"}), "positive"),
        (json!({"numerator": "-0", "denominator": "1"}), "integer"),
        (
            json!({"numerator": "1".repeat(39), "denominator": "1"}),
            "integer",
        ),
        (json!({"numerator": 1, "denominator": "1"}), "integer"),
        (
            json!({"numerator": "1", "denominator": "1", "unit": "byte"}),
            "unit",
        ),
    ] {
        let mut tagged = value.clone();
        tagged["type"] = json!("exact_rational");
        let error = check_exact_rational(&tagged).unwrap_err();
        assert_eq!(error.code(), "encoding", "{value}");
        assert!(error.to_string().contains(rule), "{value}: {error}");
    }

    // Edition-two definitions admit at schema version 2 with validate.py's digests.
    for name in ["northstar", "spend-review"] {
        let definition = &bundle(name)["definition"];
        assert_eq!(definition["schema_version"], 2, "{name}");
        let admitted = admit(definition).unwrap();
        assert_eq!(admitted.digest, definition["digest"], "{name}");
        assert_eq!(admitted.schema_version, 2, "{name}");
    }

    // Coverage on every edition-two input, complete for records.
    let refuse = |pointer: &str, value: Option<Value>, code: &str, rule: &str| {
        let mut definition = northstar_definition();
        set(&mut definition, pointer, value);
        rehash(&mut definition);
        let error = admit(&definition).unwrap_err();
        assert_eq!(error.code(), code, "{pointer}: {error}");
        assert!(error.to_string().contains(rule), "{pointer}: {error}");
    };
    refuse(
        "/content/inputs/0/coverage",
        None,
        "encoding",
        "$.content.inputs[0].coverage",
    );
    refuse(
        "/content/inputs/0/coverage",
        Some(json!("unknown")),
        "encoding",
        "$.content.inputs[0].coverage",
    );
    refuse(
        "/content/inputs/2/coverage",
        Some(json!("partial")),
        "encoding",
        "records input requires complete coverage",
    );
    let mut partial = northstar_definition();
    set(
        &mut partial,
        "/content/inputs/0/coverage",
        Some(json!("partial")),
    );
    rehash(&mut partial);
    assert!(admit(&partial).is_ok());

    // The window-derived denominator needs a whole-minute window.
    let rate = "/content/views/5/query";
    let definition = northstar_definition();
    assert_eq!(
        definition.pointer(&format!("{rate}/operations/1/denominator")),
        Some(&json!({"type": "window_minutes"}))
    );
    refuse(
        &format!("{rate}/window"),
        None,
        "encoding",
        "window_minutes denominator requires a window",
    );
    refuse(
        &format!("{rate}/window/end"),
        Some(json!("2026-10-22T10:10:30Z")),
        "encoding",
        "window must be a whole number of minutes",
    );
    refuse(
        &format!("{rate}/operations/1/denominator"),
        Some(json!({"type": "window_minutes", "minutes": 10})),
        "unknown_field",
        "denominator.minutes",
    );
    let mut spanning = northstar_definition();
    set(
        &mut spanning,
        &format!("{rate}/window"),
        Some(
            json!({"start": "2024-02-28T23:59:00Z", "end": "2024-03-01T00:01:00Z", "bounds": "half_open"}),
        ),
    );
    rehash(&mut spanning);
    assert!(admit(&spanning).is_ok());

    // Each schema version admits only its own shape.
    let mut edition_one_shape = retained("northstar")["definition"].clone();
    edition_one_shape["schema_version"] = json!(2);
    let error = admit(&edition_one_shape).unwrap_err();
    assert_eq!(error.code(), "encoding");
    assert!(
        error.to_string().contains("$.content.inputs[0].coverage"),
        "{error}"
    );
    let mut ahead = northstar_definition();
    ahead["schema_version"] = json!(3);
    assert_eq!(admit(&ahead).unwrap_err().code(), "encoding");
}

#[test]
fn edition_one_definitions_still_admit() {
    let lock: Value = serde_json::from_str(LOCK).unwrap();
    for (path, identity) in EDITION_ONE {
        let (_, text) = FIXTURES.iter().find(|(name, _)| *name == path).unwrap();
        assert_eq!(hex(&Sha256::digest(text.as_bytes())), identity, "{path}");
        assert_eq!(lock["artifacts"][path], identity, "{path}");
    }

    // Every edition-one vector is retained verbatim and reproduces unchanged.
    let current = locked("contracts/fixtures/canonical-vectors.json");
    let vectors = retained("canonical-vectors");
    assert_eq!(vectors.as_array().unwrap().len(), 6);
    for vector in vectors.as_array().unwrap() {
        let name = vector["name"].as_str().unwrap();
        assert!(current.as_array().unwrap().contains(vector), "{name}");
        let canonical = encode(&vector["value"]).unwrap();
        assert_eq!(hex(&canonical), vector["utf8_hex"], "{name}");
        assert_eq!(
            digest(digest::RESULT, &canonical),
            vector["digest"],
            "{name}"
        );
    }

    // Stored edition-one definitions admit unchanged at schema version 1.
    for name in ["northstar", "spend-review"] {
        let bundle = retained(name);
        let definition = &bundle["definition"];
        let admitted = admit(definition).unwrap();
        assert_eq!(admitted.digest, definition["digest"], "{name}");
        assert_eq!(bundle["publication"]["definition_digest"], admitted.digest);
        assert_eq!(bundle["receipt"]["definition_digest"], admitted.digest);
        assert_eq!(
            (
                admitted.schema_version,
                admitted.renderer_version,
                admitted.operator_version
            ),
            (1, 1, 1),
            "{name}"
        );
        let contracts = bundle["publication"]["input_contract_digests"]
            .as_array()
            .unwrap();
        for (input, contract) in definition["content"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .zip(contracts)
        {
            assert_eq!(
                contract["digest"],
                digest(digest::INPUT_CONTRACT, &encode(input).unwrap())
            );
        }
    }

    // Edition-one negative vectors keep their verdicts against edition-one bundles.
    assert_eq!(
        negative_vectors(
            "contracts/fixtures/impl-2026-10-28.1/negative-vectors.json",
            retained
        ),
        19
    );

    // An edition-two member is not part of an edition-one definition.
    let mut definition = retained("northstar")["definition"].clone();
    definition["content"]["inputs"][0]["coverage"] = json!("complete");
    rehash(&mut definition);
    assert_eq!(
        admit(&definition),
        Err(UnknownField("$.content.inputs[0].coverage".into()))
    );
    let mut definition = retained("northstar")["definition"].clone();
    definition["content"]["views"][0]["query"] = json!({
        "operator_version": 1, "inputs": ["events"], "null_policy": "unknown",
        "window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"},
        "operations": [
            {"type": "aggregate", "group_by": [], "measures": [{"id": "total_bytes", "operation": "sum", "field": {"input_id": "events", "field_id": "bytes"}}]},
            {"type": "ratio", "numerator": "total_bytes", "denominator": {"type": "window_minutes"}, "output_field": "rate", "output_unit": "byte_per_minute", "zero_denominator": "error", "rounding": "exact_rational"}
        ]
    });
    rehash(&mut definition);
    let error = admit(&definition).unwrap_err();
    assert_eq!(error.code(), "encoding");
    assert!(
        error.to_string().contains("operations[1].denominator.type"),
        "{error}"
    );
}
