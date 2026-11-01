//! Additive APP-158/159 receipt grammar. It is an identity document, never an
//! authority token. Existing source-query receipt domains and vectors are unchanged.
use super::{encode, AdmissionError};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
fn fail() -> AdmissionError {
    AdmissionError::Encoding("$.record_components_receipt: closed receipt refused".into())
}
fn keys(v: &Value, expected: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
fn id(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 36
            && s.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
            && matches!(s.as_bytes()[14], b'4' | b'7')
            && matches!(s.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
    })
}
fn local(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        (1..=64).contains(&s.len())
            && s.as_bytes()[0].is_ascii_lowercase()
            && s.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    })
}
fn positive(v: &Value) -> bool {
    v.as_i64()
        .is_some_and(|i| (1..=9_007_199_254_740_991).contains(&i))
}
fn digest(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 71
            && s.starts_with("sha256:")
            && s[7..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub fn admit_record_components_receipt(v: &Value) -> Result<(), AdmissionError> {
    if !keys(
        v,
        &[
            "kind",
            "schema_version",
            "organization_id",
            "app_id",
            "work_id",
            "publication_id",
            "definition_digest",
            "work_version",
            "policy_epoch",
            "actor",
            "operator_version",
            "parameters",
            "components",
            "snapshot",
            "snapshot_digest",
            "result_digest",
            "partial",
            "total_scope",
            "digest",
        ],
    ) || v["kind"] != "record_components_receipt"
        || v["schema_version"] != 1
        || v["operator_version"] != 1
        || v["partial"] != false
        || v["total_scope"] != "full_authorized_filtered_collection"
    {
        return Err(fail());
    }
    if !["organization_id", "app_id", "work_id", "publication_id"]
        .iter()
        .all(|k| id(&v[k]))
        || ![
            "definition_digest",
            "snapshot_digest",
            "result_digest",
            "digest",
        ]
        .iter()
        .all(|k| digest(&v[k]))
        || !positive(&v["work_version"])
        || !positive(&v["policy_epoch"])
    {
        return Err(fail());
    }
    let a = &v["actor"];
    if !(keys(a, &["kind", "principal_id", "membership_epoch"])
        && a["kind"] == "member"
        && id(&a["principal_id"])
        && positive(&a["membership_epoch"])
        || keys(a, &["kind", "account_id", "share_id", "share_version"])
            && a["kind"] == "guest"
            && id(&a["account_id"])
            && id(&a["share_id"])
            && positive(&a["share_version"]))
    {
        return Err(fail());
    }
    let components = v["components"].as_array().ok_or_else(fail)?;
    if components.len() > 32 {
        return Err(fail());
    }
    let mut component_ids = BTreeSet::new();
    for c in components {
        if !keys(
            c,
            &["view_id", "kind", "population", "truncated_page", "budget"],
        ) || !local(&c["view_id"])
            || !component_ids.insert(c["view_id"].as_str().unwrap())
            || !matches!(
                c["kind"].as_str(),
                Some(
                    "records_list"
                        | "record_summary"
                        | "record_trend"
                        | "record_ranking"
                        | "record_budget"
                )
            )
            || c["population"].as_u64().is_none_or(|n| n > 2000)
            || !c["truncated_page"].is_boolean()
        {
            return Err(fail());
        }
        let budget = &c["budget"];
        if c["kind"] == "record_budget" {
            if !keys(
                budget,
                &["origin", "minor_units", "currency", "minor_unit_exponent"],
            ) || budget["origin"] != "preset"
                || budget["currency"] != "USD"
                || budget["minor_unit_exponent"] != 2
                || !budget["minor_units"].as_str().is_some_and(|s| {
                    !s.is_empty()
                        && s.len() <= 38
                        && s.bytes().all(|b| b.is_ascii_digit())
                        && (s == "0" || !s.starts_with('0'))
                })
            {
                return Err(fail());
            }
        } else if !budget.is_null() {
            return Err(fail());
        }
    }
    let p = &v["parameters"];
    if !keys(p, &["filter_epoch", "filters"])
        || !p["filter_epoch"]
            .as_i64()
            .is_some_and(|i| (0..=9_007_199_254_740_991).contains(&i))
    {
        return Err(fail());
    }
    let fs = p["filters"].as_array().ok_or_else(fail)?;
    if fs.len() > 8 {
        return Err(fail());
    }
    let mut seen = BTreeSet::new();
    for f in fs {
        if !local(&f["id"]) || !seen.insert(f["id"].as_str().unwrap()) {
            return Err(fail());
        }
        let text = keys(f, &["id", "value"])
            && f["value"]
                .as_str()
                .is_some_and(|s| (1..=200).contains(&s.chars().count()));
        let dates = keys(f, &["id", "start", "end"])
            && ["start", "end"].iter().all(|k| {
                f[k].as_str().is_some_and(|s| {
                    s.len() == 10 && super::contract_timestamp(&format!("{s}T00:00:00Z"))
                })
            })
            && f["start"].as_str() < f["end"].as_str();
        if !text && !dates {
            return Err(fail());
        }
    }
    let snapshots = v["snapshot"].as_array().ok_or_else(fail)?;
    if snapshots.len() > 32 {
        return Err(fail());
    }
    let mut inputs = BTreeSet::new();
    let mut records = BTreeSet::new();
    let mut total = 0;
    for s in snapshots {
        if !keys(
            s,
            &[
                "input_id",
                "collection_id",
                "schema_id",
                "schema_version",
                "records",
            ],
        ) || !local(&s["input_id"])
            || !local(&s["schema_id"])
            || !id(&s["collection_id"])
            || !positive(&s["schema_version"])
            || !inputs.insert(s["input_id"].as_str().unwrap())
        {
            return Err(fail());
        }
        for r in s["records"].as_array().ok_or_else(fail)? {
            total += 1;
            if total > 2000
                || !keys(r, &["record_id", "version", "commit_sequence"])
                || !id(&r["record_id"])
                || !positive(&r["version"])
                || !positive(&r["commit_sequence"])
                || !records.insert((
                    s["input_id"].as_str().unwrap(),
                    s["collection_id"].as_str().unwrap(),
                    r["record_id"].as_str().unwrap(),
                ))
            {
                return Err(fail());
            }
        }
    }
    let mut body = v.as_object().ok_or_else(fail)?.clone();
    body.remove("digest");
    let mut h = Sha256::new();
    h.update(b"forma:record-components-receipt:v1\0");
    h.update(encode(&Value::Object(body))?);
    let computed = format!(
        "sha256:{}",
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    if v["digest"] != computed {
        return Err(fail());
    }
    Ok(())
}
