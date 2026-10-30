//! APP-012 authorized typed query (design §§6.3, 7.2; C06, P06; ADR-007
//! D7.10, D7.9; ADR-004 order). Plan Task 9's `Data::run_query` and
//! `Data::release_result`.
//!
//! `run_query` is one READ COMMITTED transaction on the data role in ADR-004
//! order: the member precheck (current membership, epoch and trial
//! entitlement, then `record_read` on the saved work, which includes `run` on
//! its app and live `source_read` on every dependency), the `genui_lock`
//! fence, the saved work's pinned publication and version under
//! `lock_work_revision`, the publication's stored definition re-admitted
//! (APP-010), the view's typed query, the bound source revision of its one
//! input under that member's own `source_read`, and the record snapshot
//! sequence. It returns a [`QueryRun`]: everything the engine child and the
//! receipt need, and nothing a caller can forge (no public constructor).
//!
//! Authorization is judged before engine admission (this transaction: the
//! engine only ever receives a `QueryRun`), at execution (the engine child
//! re-checks the cached revision's digest) and at delivery by
//! `release_result`, a second precheck and fence that must still see the same
//! publication, work version, binding, live revision and policy version, so a
//! revocation or a foreign scope between execution and delivery denies the
//! release (C07).
//!
//! Parameters are closed (`parse_parameters`): an optional half-open
//! `window` on the events time field, an optional `destination` equality
//! filter and the client's `filter_epoch`. They are applied to the view's
//! stored query here, so the engine receives one effective typed query whose
//! digest is the receipt's `predicate_digest`; the stored view query's digest
//! is its `query_digest`. No money, ratio or lookup is ever added.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::stored_content,
    pool::Data,
    require, valid_id, Error, Result, TrustedRequest,
};
use crate::admission::{self, digest};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline; each transaction here stays inside it.
pub const QUERY_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// Largest safe integer (forma-json-v1) for the client's filter epoch.
const MAX_SAFE: i64 = 9_007_199_254_740_991;

/// The one source revision a Phase 1 query reads (APP-005's qualified subset
/// has one registered input).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSource {
    input: String,
    source: String,
    revision: String,
    normalized_digest: String,
}
impl RunSource {
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    /// SHA-256 of the revision's `part.parquet` (ADR-009 D9.3), as stored in
    /// `source_revisions` and bound by the work.
    pub fn normalized_digest(&self) -> &str {
        &self.normalized_digest
    }
    /// ADR-009 B1 object key of the normalized revision.
    pub fn object_key(&self, organization: &str) -> String {
        format!(
            "o/{organization}/s/{}/r/{}/part.parquet",
            self.source, self.revision
        )
    }
}

/// An authorized, not yet released query: the identities its receipt pins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryRun {
    organization: String,
    principal: String,
    membership_epoch: i64,
    app: String,
    work: String,
    work_version: i64,
    publication: String,
    definition_digest: String,
    policy_epoch: i64,
    view: String,
    view_query: Value,
    query: Value,
    parameters: Vec<Value>,
    filter_epoch: i64,
    record_snapshot_sequence: i64,
    source: RunSource,
}
impl QueryRun {
    pub fn organization(&self) -> &str {
        &self.organization
    }
    pub fn principal(&self) -> &str {
        &self.principal
    }
    pub fn membership_epoch(&self) -> i64 {
        self.membership_epoch
    }
    pub fn app(&self) -> &str {
        &self.app
    }
    pub fn work(&self) -> &str {
        &self.work
    }
    pub fn work_version(&self) -> i64 {
        self.work_version
    }
    pub fn publication(&self) -> &str {
        &self.publication
    }
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }
    /// The publication's policy version: the policy epoch the run was
    /// authorized under (0002 keeps no separate policy epoch).
    pub fn policy_epoch(&self) -> i64 {
        self.policy_epoch
    }
    pub fn view(&self) -> &str {
        &self.view
    }
    /// The view's stored typed query (`provenance.query_digest`).
    pub fn view_query(&self) -> &Value {
        &self.view_query
    }
    /// The effective typed query sent to the engine (`predicate_digest`).
    pub fn query(&self) -> &Value {
        &self.query
    }
    /// Receipt `parameters`: named values in `field_id` order.
    pub fn parameters(&self) -> &[Value] {
        &self.parameters
    }
    pub fn filter_epoch(&self) -> i64 {
        self.filter_epoch
    }
    pub fn record_snapshot_sequence(&self) -> i64 {
        self.record_snapshot_sequence
    }
    pub fn source(&self) -> &RunSource {
        &self.source
    }

    /// A run for engine and receipt unit tests only; never reachable from a
    /// request (the test executable only).
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_tests(
        organization: &str,
        principal: &str,
        app: &str,
        work: &str,
        publication: &str,
        definition_digest: &str,
        view: &str,
        view_query: Value,
        parameters: &Value,
        source: (&str, &str, &str),
    ) -> Self {
        let parsed = parse_parameters(parameters).expect("test parameters");
        let query = parsed.apply(&view_query).expect("test query");
        Self {
            organization: organization.into(),
            principal: principal.into(),
            membership_epoch: 1,
            app: app.into(),
            work: work.into(),
            work_version: 1,
            publication: publication.into(),
            definition_digest: definition_digest.into(),
            policy_epoch: 1,
            view: view.into(),
            view_query,
            query,
            parameters: parsed.named_values(),
            filter_epoch: parsed.filter_epoch,
            record_snapshot_sequence: 0,
            source: RunSource {
                input: "events".into(),
                source: source.0.into(),
                revision: source.1.into(),
                normalized_digest: source.2.into(),
            },
        }
    }
}

/// The closed run parameters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Parameters {
    pub window: Option<(String, String)>,
    pub destination: Option<String>,
    pub filter_epoch: i64,
}

/// Contract `TIMESTAMP`: whole-second UTC `YYYY-MM-DDTHH:MM:SSZ` with valid
/// field ranges (the engine compares it as a naive UTC timestamp).
fn timestamp(value: &str) -> bool {
    let b = value.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        b.get(range.clone())?
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| value[range].parse().ok())?
    };
    b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && digits(0..4).is_some()
        && digits(5..7).is_some_and(|m| (1..=12).contains(&m))
        && digits(8..10).is_some_and(|d| (1..=31).contains(&d))
        && digits(11..13).is_some_and(|h| h < 24)
        && digits(14..16).is_some_and(|m| m < 60)
        && digits(17..19).is_some_and(|s| s < 60)
}

/// Parses the closed parameter object; `None` for anything else (the route
/// answers 422 before calling `run_query`, which refuses it as `Denied`).
pub fn parse_parameters(value: &Value) -> Option<Parameters> {
    let object = value.as_object()?;
    if !object
        .keys()
        .all(|k| matches!(k.as_str(), "window" | "destination" | "filter_epoch"))
    {
        return None;
    }
    let mut parameters = Parameters::default();
    if let Some(window) = object.get("window") {
        let window = window.as_object()?;
        if window.len() != 2 {
            return None;
        }
        let start = window.get("start")?.as_str()?;
        let end = window.get("end")?.as_str()?;
        if !(timestamp(start) && timestamp(end) && start < end) {
            return None;
        }
        parameters.window = Some((start.into(), end.into()));
    }
    if let Some(destination) = object.get("destination") {
        let destination = destination.as_str()?;
        if destination.is_empty() || destination.chars().count() > 2000 {
            return None;
        }
        parameters.destination = Some(destination.into());
    }
    if let Some(epoch) = object.get("filter_epoch") {
        parameters.filter_epoch = epoch.as_i64().filter(|e| (0..=MAX_SAFE).contains(e))?;
    }
    Some(parameters)
}

impl Parameters {
    /// Receipt named values, in `field_id` order.
    fn named_values(&self) -> Vec<Value> {
        let mut values = Vec::new();
        if let Some(destination) = &self.destination {
            values.push(
                json!({"field_id": "destination", "value": {"type": "text", "value": destination}}),
            );
        }
        if let Some((start, end)) = &self.window {
            values.push(
                json!({"field_id": "window_end", "value": {"type": "timestamp", "value": end}}),
            );
            values.push(
                json!({"field_id": "window_start", "value": {"type": "timestamp", "value": start}}),
            );
        }
        values
    }
    /// The effective query: the stored view query with the window and the
    /// destination filter applied. A view that declares its own window takes
    /// no window parameter.
    fn apply(&self, query: &Value) -> Option<Value> {
        let mut query = query.as_object()?.clone();
        if let Some((start, end)) = &self.window {
            if query.contains_key("window") {
                return None;
            }
            query.insert(
                "window".into(),
                json!({"start": start, "end": end, "bounds": "half_open"}),
            );
        }
        if let Some(destination) = &self.destination {
            let operations = query.get_mut("operations")?.as_array_mut()?;
            operations.insert(
                0,
                json!({"type": "filter", "field": {"input_id": "events", "field_id": "destination"},
                    "comparison": "eq", "value": {"type": "text", "value": destination}}),
            );
        }
        Some(Value::Object(query))
    }
}

fn local_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}
fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// The view's typed query in the re-admitted definition content.
fn view_query(content: &[u8], view: &str) -> Result<Value> {
    let content = admission::decode(content).map_err(|_| Error::Unavailable)?;
    let views = content
        .get("views")
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?;
    let found = views
        .iter()
        .find(|v| v.get("id").and_then(Value::as_str) == Some(view))
        .ok_or(Error::Denied)?;
    // A view without a query (form, note) has nothing to run.
    found.get("query").cloned().ok_or(Error::Denied)
}

/// The pinned premise a run and its release must both see.
struct Premise {
    publication: String,
    work_version: i64,
    definition_digest: String,
    policy_epoch: i64,
    definition_revision: String,
}
async fn premise(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
) -> Result<Premise> {
    let pin = transaction.query("SELECT publication_id::text,version::bigint FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&organization,&app,&work]).await?;
    let pin = pin.first().ok_or(Error::Denied)?;
    let (publication, version) = (text(pin, 0)?, integer(pin, 1)?);
    let locked = transaction.query("SELECT publication_id::text,work_version FROM genui_private.lock_work_revision($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5)", &[&organization,&app,&work,&publication,&version]).await?;
    let locked = locked.first().ok_or(Error::Denied)?;
    require(text(locked, 0)? == publication && integer(locked, 1)? == version)?;
    let rows = transaction.query("SELECT definition_revision_id::text,definition_digest::text,policy_version::bigint,operator_version FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[&organization,&app,&publication]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    let operator: i32 = row.try_get(3).map_err(|_| Error::Unavailable)?;
    if operator != 1 {
        return Err(Error::Unavailable);
    }
    Ok(Premise {
        publication,
        work_version: version,
        definition_revision: text(row, 0)?,
        definition_digest: text(row, 1)?,
        policy_epoch: integer(row, 2)?,
    })
}

/// The work's binding of `input`, which must be a live source revision whose
/// stored `normalized_digest` the work bound.
async fn bound_source(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    input: &str,
) -> Result<RunSource> {
    let rows = transaction.query("SELECT b.kind,b.source_id::text,b.source_revision_id::text,b.source_digest::text,r.normalized_digest::text FROM genui.work_bindings b LEFT JOIN genui.source_revisions r ON r.organization_id=b.organization_id AND r.source_id=b.source_id AND r.revision_id=b.source_revision_id AND r.deleted_at IS NULL WHERE b.organization_id=$1::text::uuid AND b.app_id=$2::text::uuid AND b.work_id=$3::text::uuid AND b.input_id=$4::text", &[&organization,&app,&work,&input]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    require(text(row, 0)? == "source")?;
    let optional =
        |index| -> Result<Option<String>> { row.try_get(index).map_err(|_| Error::Unavailable) };
    let (source, revision, bound) = (
        optional(1)?.ok_or(Error::Unavailable)?,
        optional(2)?.ok_or(Error::Unavailable)?,
        optional(3)?.ok_or(Error::Unavailable)?,
    );
    // A deleted revision is no longer readable: a denial, never a stale number.
    let live = optional(4)?.ok_or(Error::Denied)?;
    require(live == bound)?;
    Ok(RunSource {
        input: input.into(),
        source,
        revision,
        normalized_digest: bound,
    })
}

impl Data {
    /// Authorizes and pins one run of `view` on saved work `work` of `app`
    /// for the requesting member. Nothing executes here and nothing is
    /// written. `Denied` covers an unknown or foreign app, work or view, a
    /// view with no query, a binding that is not one live source revision,
    /// malformed parameters and every missing authority alike.
    pub async fn run_query(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        view: &str,
        parameters: &Value,
    ) -> Result<QueryRun> {
        require(valid_id(app) && valid_id(work) && local_id(view))?;
        let parameters = parse_parameters(parameters).ok_or(Error::Denied)?;
        let cap = Instant::now() + QUERY_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let premise = premise(transaction, org, app, work).await?;
            let (admitted, content) =
                stored_content(transaction, org, app, &premise.definition_revision).await?;
            if admitted != premise.definition_digest {
                return Err(Error::Unavailable);
            }
            let stored = view_query(&content, view)?;
            let inputs = stored
                .get("inputs")
                .and_then(Value::as_array)
                .ok_or(Error::Unavailable)?;
            // APP-005's qualified subset reads exactly one source input.
            require(inputs.len() == 1)?;
            let input = inputs[0].as_str().ok_or(Error::Unavailable)?;
            let source = bound_source(transaction, org, app, work, input).await?;
            permit
                .also(Action::SourceRead, Resource::Source(&source.source))
                .await?;
            permit.also(Action::Run, Resource::App(app)).await?;
            let rows = transaction.query("SELECT coalesce(max(commit_sequence),0)::bigint FROM genui.internal_record_revisions WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[org,&app,&work]).await?;
            let record_snapshot_sequence = integer(rows.first().ok_or(Error::Unavailable)?, 0)?;
            let query = parameters.apply(&stored).ok_or(Error::Denied)?;
            Ok(QueryRun {
                organization: org.clone(),
                principal: request.principal.clone(),
                membership_epoch: request.epoch,
                app: app.into(),
                work: work.into(),
                work_version: premise.work_version,
                publication: premise.publication,
                definition_digest: premise.definition_digest,
                policy_epoch: premise.policy_epoch,
                view: view.into(),
                view_query: stored,
                query,
                parameters: parameters.named_values(),
                filter_epoch: parameters.filter_epoch,
                record_snapshot_sequence,
                source,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The second precheck and fence before delivery (design §6.3, C07): the
    /// same member, at the same epoch, with current entitlement, `record_read`
    /// on the work, `run` on the app and `source_read` on the source, must
    /// still see the run's publication, work version, definition, policy
    /// version, binding and live revision. Any change is `Denied` and the
    /// result is never released.
    pub async fn release_result(&self, request: &TrustedRequest, run: &QueryRun) -> Result<()> {
        require(
            run.organization == request.organization
                && run.principal == request.principal
                && run.membership_epoch == request.epoch,
        )?;
        let (app, work) = (run.app.as_str(), run.work.as_str());
        let cap = Instant::now() + QUERY_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let premise = premise(transaction, org, app, work).await?;
            require(
                premise.publication == run.publication
                    && premise.work_version == run.work_version
                    && premise.definition_digest == run.definition_digest
                    && premise.policy_epoch == run.policy_epoch,
            )?;
            let source = bound_source(transaction, org, app, work, &run.source.input).await?;
            require(source == run.source)?;
            permit
                .also(Action::SourceRead, Resource::Source(&source.source))
                .await?;
            permit.also(Action::Run, Resource::App(app)).await
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

/// The receipt's stored-view and effective-query identities (`forma:query:v1`).
pub fn query_digests(run: &QueryRun) -> Option<(String, String)> {
    let view = admission::encode(&run.view_query).ok()?;
    let effective = admission::encode(&run.query).ok()?;
    Some((
        digest::digest(digest::QUERY, &view),
        digest::digest(digest::QUERY, &effective),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_are_closed_and_validated() {
        let window =
            json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}});
        let parsed = parse_parameters(&window).unwrap();
        assert_eq!(
            parsed.window,
            Some(("2026-10-22T10:00:00Z".into(), "2026-10-22T10:10:00Z".into()))
        );
        assert_eq!(parse_parameters(&json!({})), Some(Parameters::default()));
        for bad in [
            json!([]),
            json!({"sql": "SELECT 1"}),
            json!({"window": {"start": "2026-10-22T10:10:00Z", "end": "2026-10-22T10:00:00Z"}}),
            json!({"window": {"start": "2026-10-22T10:00:00+01:00", "end": "2026-10-22T10:10:00Z"}}),
            json!({"window": {"start": "2026-13-22T10:00:00Z", "end": "2026-14-22T10:10:00Z"}}),
            json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "closed"}}),
            json!({"destination": ""}),
            json!({"destination": 7}),
            json!({"filter_epoch": -1}),
            json!({"filter_epoch": 9_007_199_254_740_992_i64}),
            json!({"amount_cents": {"type": "money", "minor_units": "20000"}}),
        ] {
            assert_eq!(parse_parameters(&bad), None, "{bad}");
        }
    }

    #[test]
    fn effective_query_adds_window_and_destination_only() {
        let total = json!({"operator_version": 1, "inputs": ["events"], "null_policy": "unknown",
            "operations": [{"type": "aggregate", "group_by": [], "measures": [{"id": "total_bytes", "operation": "sum", "field": {"input_id": "events", "field_id": "bytes"}}]}]});
        let parameters = parse_parameters(&json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}, "destination": "198.51.100.7", "filter_epoch": 3})).unwrap();
        let effective = parameters.apply(&total).unwrap();
        assert_eq!(
            effective["window"],
            json!({"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"})
        );
        assert_eq!(effective["operations"][0]["type"], "filter");
        assert_eq!(effective["operations"][1], total["operations"][0]);
        assert_eq!(
            parameters
                .named_values()
                .iter()
                .map(|v| v["field_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["destination", "window_end", "window_start"]
        );
        let mut windowed = total.clone();
        windowed["window"] = effective["window"].clone();
        assert_eq!(parameters.apply(&windowed), None);
    }
}
