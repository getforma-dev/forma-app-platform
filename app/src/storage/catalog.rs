//! The migration owner's canonical logical catalog contract, embedded at build
//! time. No request or runtime path can provide an alternative SQL statement.
use super::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use tokio_postgres::Client;

const QUERY: &str = include_str!("../../migrations/catalog-query.json");
const EXPECTED: &str = include_str!("../../migrations/catalog-v11.json");
const BOUND: usize = 524_288;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    search_path: String,
    max_output_bytes: usize,
    section_row_limit: usize,
    sql: String,
}
pub(super) struct Catalog {
    sql: String,
    expected: Value,
}
impl Catalog {
    pub fn embedded() -> Result<Self> {
        if QUERY.len() > BOUND || EXPECTED.len() > BOUND {
            return Err(Error::Configuration);
        }
        let query: Document = serde_json::from_str(QUERY).map_err(|_| Error::Configuration)?;
        let expected: Value = serde_json::from_str(EXPECTED).map_err(|_| Error::Configuration)?;
        if query.schema_version != 1
            || query.search_path != "pg_catalog, pg_temp"
            || query.max_output_bytes != BOUND
            || query.section_row_limit != 1025
            || !expected.is_object()
            || query.sql.is_empty()
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            sql: query.sql,
            expected,
        })
    }
    pub async fn verify(&self, client: &Client) -> Result<()> {
        // A failed/cancelled check leaves the surrounding owned connection
        // dirty; its admission/recycle guard discards the transport.
        client
            .batch_execute("BEGIN READ ONLY; SET LOCAL search_path=pg_catalog,pg_temp")
            .await
            .map_err(|_| Error::Unavailable)?;
        let row = client
            .query_one(&self.sql, &[])
            .await
            .map_err(|_| Error::Unavailable)?;
        let body = row
            .try_get::<_, Option<String>>(0)
            .map_err(|_| Error::Unavailable)?
            .ok_or(Error::Unavailable)?;
        if body.len() > BOUND {
            return Err(Error::Unavailable);
        }
        let actual: Value = serde_json::from_str(&body).map_err(|_| Error::Unavailable)?;
        if actual != self.expected {
            return Err(Error::Unavailable);
        }
        client
            .batch_execute("ROLLBACK")
            .await
            .map_err(|_| Error::Unavailable)
    }
}
