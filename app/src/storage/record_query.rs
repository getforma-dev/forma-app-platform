//! Full, exact-work record snapshots for the existing typed query compiler.
//! Bounds apply before row bytes leave PostgreSQL; no prefix is ever a total.
use super::{authorization::Transaction, require, Error, Result};
use crate::admission;
use serde_json::{Map, Value};
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const MAX_ROWS: i64 = 100_000;
pub const MAX_BYTES: i64 = 64 * 1024 * 1024;
static SNAPSHOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct RecordQuerySnapshot {
    pub(super) input: String,
    pub(super) collection: String,
    pub(super) schema: String,
    pub(super) schema_version: i64,
    pub(super) last_commit: i64,
    pub(super) sequence: i64,
    rows: Arc<Vec<u8>>,
    // Held through engine admission/execution/release, including clones. At
    // most four full snapshots can occupy application memory at one time.
    _slot: Arc<OwnedSemaphorePermit>,
}
impl PartialEq for RecordQuerySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input
            && self.collection == other.collection
            && self.schema == other.schema
            && self.schema_version == other.schema_version
            && self.last_commit == other.last_commit
            && self.sequence == other.sequence
            && self.rows == other.rows
    }
}
impl Eq for RecordQuerySnapshot {}
impl RecordQuerySnapshot {
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn bytes(&self) -> &[u8] {
        &self.rows
    }
}

pub(super) async fn binding(
    tx: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    input: &str,
) -> Result<(String, String, i64)> {
    let rows=tx.query("SELECT collection_id::text,schema_id::text,schema_version::bigint FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND input_id=$4::text AND kind='records' AND revision_policy='committed_snapshot'", &[&org,&app,&work,&input]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    Ok((
        row.try_get(0).map_err(|_| Error::Unavailable)?,
        row.try_get(1).map_err(|_| Error::Unavailable)?,
        row.try_get(2).map_err(|_| Error::Unavailable)?,
    ))
}

fn scalar_row(bytes: &[u8], fields: &[Value]) -> Result<Value> {
    let value = admission::decode(bytes).map_err(|_| Error::Unavailable)?;
    let values = value.as_array().ok_or(Error::Unavailable)?;
    require(values.len() == fields.len())?;
    let mut row = Map::new();
    for field in fields {
        let id = field["id"].as_str().ok_or(Error::Unavailable)?;
        let values: Vec<_> = values.iter().filter(|v| v["field_id"] == id).collect();
        require(values.len() == 1)?;
        let value = &values[0]["value"];
        let kind = value["type"].as_str().ok_or(Error::Unavailable)?;
        let spec = &field["value_type"];
        if kind == "null" {
            require(field["nullable"] == true)?;
            row.insert(id.into(), Value::Null);
            continue;
        }
        require(value["type"] == spec["type"])?;
        let scalar = match kind {
            "integer" => {
                require(value["unit"] == spec["unit"])?;
                &value["value"]
            }
            "decimal" => {
                require(value["scale"] == spec["scale"] && value["unit"] == spec["unit"])?;
                &value["coefficient"]
            }
            "money" => {
                require(
                    value["currency"] == spec["currency"]
                        && value["minor_unit_exponent"] == spec["minor_unit_exponent"],
                )?;
                &value["minor_units"]
            }
            "text" | "enum" | "date" | "timestamp" | "boolean" => &value["value"],
            _ => return Err(Error::Denied),
        };
        require(if kind == "boolean" {
            scalar.is_boolean()
        } else {
            scalar.is_string()
        })?;
        row.insert(id.into(), scalar.clone());
    }
    Ok(Value::Object(row))
}

pub(super) async fn capture(
    tx: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    input: &str,
    fields: &[Value],
) -> Result<RecordQuerySnapshot> {
    let slot = SNAPSHOTS
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::Busy)?;
    let (collection, schema, schema_version) = binding(tx, org, app, work, input).await?;
    // Metadata, organization sequence, and row versions are from ONE SQL
    // statement snapshot, including under a READ COMMITTED member transaction.
    // The bounds CTE reads lengths only; the guarded lateral read fetches no
    // canonical row bytes at all when the complete collection exceeds a bound.
    let rows=tx.query("WITH bounds AS MATERIALIZED (SELECT count(*)::bigint AS n,coalesce(sum(octet_length(v.canonical_values)),0)::bigint AS bytes,coalesce(max(v.commit_sequence),0)::bigint AS last FROM genui.internal_records r JOIN genui.internal_record_revisions v USING(organization_id,app_id,work_id,collection_id,record_id) WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND v.version=r.current_version) SELECT b.n,b.bytes,b.last,(SELECT next_commit_sequence-1 FROM genui.organization_sequences WHERE organization_id=$1::text::uuid)::bigint,d.canonical_values FROM bounds b LEFT JOIN LATERAL (SELECT v.canonical_values,r.record_id FROM genui.internal_records r JOIN genui.internal_record_revisions v USING(organization_id,app_id,work_id,collection_id,record_id) WHERE b.n<=$5 AND b.bytes<=$6 AND r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND v.version=r.current_version ORDER BY r.record_id) d ON true ORDER BY d.record_id", &[&org,&app,&work,&collection,&MAX_ROWS,&MAX_BYTES]).await?;
    let first = rows.first().ok_or(Error::Unavailable)?;
    let number = |i| first.try_get::<_, i64>(i).map_err(|_| Error::Unavailable);
    let count = number(0)?;
    if count > MAX_ROWS || number(1)? > MAX_BYTES {
        return Err(Error::ProjectionLimit);
    }
    let last_commit = number(2)?;
    let sequence = number(3)?;
    let mut bytes = Vec::new();
    let mut actual = 0i64;
    for row in rows {
        let Some(raw) = row
            .try_get::<_, Option<Vec<u8>>>(4)
            .map_err(|_| Error::Unavailable)?
        else {
            continue;
        };
        let value = scalar_row(&raw, fields)?;
        serde_json::to_writer(&mut bytes, &value).map_err(|_| Error::Unavailable)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_BYTES as usize {
            return Err(Error::ProjectionLimit);
        }
        actual += 1;
    }
    require(actual == count && last_commit <= sequence)?;
    Ok(RecordQuerySnapshot {
        input: input.into(),
        collection,
        schema,
        schema_version,
        last_commit,
        sequence,
        rows: Arc::new(bytes),
        _slot: Arc::new(slot),
    })
}

pub(super) async fn release(
    tx: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    snapshot: &RecordQuerySnapshot,
) -> Result<()> {
    require(
        binding(tx, org, app, work, &snapshot.input).await?
            == (
                snapshot.collection.clone(),
                snapshot.schema.clone(),
                snapshot.schema_version,
            ),
    )?;
    let rows=tx.query("SELECT coalesce(max(v.commit_sequence),0)::bigint FROM genui.internal_records r JOIN genui.internal_record_revisions v USING(organization_id,app_id,work_id,collection_id,record_id) WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND v.version=r.current_version", &[&org,&app,&work,&snapshot.collection]).await?;
    let current: i64 = rows
        .first()
        .ok_or(Error::Unavailable)?
        .try_get(0)
        .map_err(|_| Error::Unavailable)?;
    require(current == snapshot.last_commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn scalar_snapshot_preserves_null_empty_exact_decimal_and_boolean() {
        let fields = json!([{"id":"a","value_type":{"type":"text"},"nullable":true},{"id":"b","value_type":{"type":"text"},"nullable":true},{"id":"n","value_type":{"type":"decimal","scale":2,"unit":"count"},"nullable":false},{"id":"flag","value_type":{"type":"boolean"},"nullable":false}]);
        let values = json!([{"field_id":"a","value":{"type":"text","value":""}},{"field_id":"b","value":{"type":"null"}},{"field_id":"n","value":{"type":"decimal","coefficient":"9007199254740993","scale":2,"unit":"count"}},{"field_id":"flag","value":{"type":"boolean","value":false}}]);
        let row = scalar_row(
            &admission::encode(&values).unwrap(),
            fields.as_array().unwrap(),
        )
        .unwrap();
        assert_eq!(
            row,
            json!({"a":"","b":null,"n":"9007199254740993","flag":false})
        );
        let mut bad = values;
        bad[2]["value"]["scale"] = json!(3);
        assert!(scalar_row(
            &admission::encode(&bad).unwrap(),
            fields.as_array().unwrap()
        )
        .is_err());
    }
}
