//! The edition-1 `query_receipt` (contracts/schema.json `$defs.query_receipt`;
//! C06, design §6.3) for a released run, and its identity
//! `digest = sha256("forma:query-receipt:v1" ‖ 0x00 ‖ forma-json-v1 of every
//! member except "digest")`.
//!
//! Mode is `published`: the run is the saved work's pinned publication. The
//! receipt pins the organization, app, work, publication and definition
//! digest; the view and operator version 1; every source dependency of the
//! work (input, source, revision, `normalized_digest`, in `input_id` order:
//! the contract's `validate_bundle` requires the receipt's `source_revisions`
//! to equal the work's `source_dependencies`, APP-012 review M3; the engine
//! still reads only the view's one input); the record snapshot sequence; the
//! parameters; the principal, membership epoch, policy epoch (the
//! publication's policy version) and filter epoch; the result digest;
//! `partial: false`, the page flag and the full-input total scope; and the
//! stored view query's and the effective query's `forma:query:v1` digests.
//!
//! Every built receipt is checked against the contract's receipt rule
//! (`admission::admit_receipt`, the Rust equivalent of `contracts/validate.py`
//! `validate` for a `query_receipt`, plus its digest) before it exists as a
//! [`QueryReceipt`]: a receipt the contract validator would refuse is never
//! delivered, and its run is treated as a failed execution with no number
//! (APP-012 review M2).
use super::worker::WorkerResult;
use crate::{
    admission::{self, digest as identity},
    storage::{query_digests, QueryRun},
};
use serde_json::{json, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryReceipt {
    /// The receipt document, `digest` included.
    pub document: Value,
    pub digest: String,
}

/// The receipt identity of a document whose other members are final.
pub fn digest(document: &Value) -> Option<String> {
    let mut body = document.as_object()?.clone();
    body.remove("digest");
    let canonical = admission::encode(&Value::Object(body)).ok()?;
    Some(identity::digest(identity::QUERY_RECEIPT, &canonical))
}

/// `contracts/validate.py` `validate` for a complete receipt document: the
/// closed contract shape and the recomputed `digest`. The error names the
/// refused location, never a value.
pub fn conforms(document: &Value) -> Result<(), String> {
    admission::admit_receipt(document).map_err(|error| format!("{}: {error:?}", error.code()))?;
    let declared = document["digest"]
        .as_str()
        .ok_or("$.digest: required field missing")?;
    if digest(document).as_deref() != Some(declared) {
        return Err("$.digest: receipt digest mismatch".into());
    }
    Ok(())
}

/// The receipt of `result` for `run`. `None` if a value cannot be encoded or
/// the built document fails the contract's receipt rule (stored state or a
/// parameter outside its contract), which the caller treats as a failed
/// execution: no receipt, no number.
pub fn build(run: &QueryRun, result: &WorkerResult) -> Option<QueryReceipt> {
    let (query_digest, predicate_digest) = query_digests(run)?;
    let source_revisions: Vec<Value> = run
        .dependencies()
        .iter()
        .map(|source| {
            json!({
                "input_id": source.input(),
                "source_id": source.source(),
                "source_revision_id": source.revision(),
                "source_digest": source.normalized_digest(),
            })
        })
        .collect();
    let mut document = json!({
        "kind": "query_receipt",
        "schema_version": 1,
        "organization_id": run.organization(),
        "app_id": run.app(),
        "work_id": run.work(),
        "mode": "published",
        "definition_digest": run.definition_digest(),
        "publication_id": run.publication(),
        "view_id": run.view(),
        "operator_version": 1,
        "source_revisions": source_revisions,
        "record_snapshot_sequence": run.record_snapshot_sequence(),
        "parameters": run.parameters(),
        "principal_id": run.principal(),
        "membership_epoch": run.membership_epoch(),
        "policy_epoch": run.policy_epoch(),
        "filter_epoch": run.filter_epoch(),
        "result_digest": result.result_digest,
        "partial": false,
        "truncated_page": result.truncated_page,
        "total_scope": "full_authorized_filtered_input",
        "provenance": {"query_digest": query_digest, "predicate_digest": predicate_digest},
    });
    let id = digest(&document)?;
    document["digest"] = json!(id);
    if let Err(reason) = conforms(&document) {
        tracing::error!(reason, "receipt outside the contract; not delivered");
        return None;
    }
    Some(QueryReceipt {
        document,
        digest: id,
    })
}
