//! The migration owner's canonical logical catalog contract, embedded at build
//! time. No request or runtime path can provide an alternative SQL statement.
//!
//! ADR-004 amendment A6 (APP-216 part 1): the database hashes its catalog
//! document and the service compares 32 bytes against the digest of the
//! pinned document in the same canonical form. Equal digests prove the live
//! document is the pinned one, so the comparison's verdict is unchanged; any
//! other answer runs the full comparison, which names the refusal as before.
use super::{pool::trace, Error, Result};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use tokio_postgres::Client;

const QUERY: &str = include_str!("../../migrations/catalog-query.json");
const EXPECTED: &str = include_str!("../../migrations/catalog-v25.json");
const BOUND: usize = 786_432;

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
    /// The same statement, hashed in the database: `sha256` of the UTF-8
    /// bytes of the document the full comparison would receive.
    /// `pg_catalog.sha256(bytea)` exists from PostgreSQL 11; the beta and the
    /// fixture run 16.
    digest_sql: String,
    expected: Value,
    /// `sha256` of `expected` in the database's own text form of the
    /// document ([`canonical`]).
    expected_digest: [u8; 32],
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
            || query.section_row_limit != 2049
            || !expected.is_object()
            || query.sql.is_empty()
        {
            return Err(Error::Configuration);
        }
        let digest_sql = format!(
            "SELECT pg_catalog.sha256(pg_catalog.convert_to(body,'UTF8')) FROM ({}) AS catalog(body)",
            query.sql
        );
        let expected_digest = Sha256::digest(canonical(&expected).as_bytes()).into();
        Ok(Self {
            sql: query.sql,
            digest_sql,
            expected,
            expected_digest,
        })
    }
    /// Test executable only: the full comparison statement and the digest
    /// statement, for the readiness traffic probe and the digest checks.
    #[cfg(test)]
    pub(super) fn statements(&self) -> (&str, &str) {
        (&self.sql, &self.digest_sql)
    }
    /// Test executable only: the digest the database must answer with.
    #[cfg(test)]
    pub(super) fn expected_digest(&self) -> [u8; 32] {
        self.expected_digest
    }
    pub async fn verify(&self, client: &Client) -> Result<()> {
        #[cfg(test)]
        VERIFIED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // A failed/cancelled check leaves the surrounding owned connection
        // dirty; its admission/recycle guard discards the transport.
        client
            .batch_execute("BEGIN READ ONLY; SET LOCAL search_path=pg_catalog,pg_temp")
            .await
            .map_err(trace::unavailable)?;
        // A6: the digest first. The database's document is jsonb text, and
        // the expected digest is the pinned document in that same text, so
        // equal digests mean the two texts are equal, and equal texts parse
        // to equal values: the verdict the full comparison would reach. A
        // NULL (the document over its bound) or a different digest runs the
        // full comparison below. A driver error on the digest statement is
        // `Unavailable` at once, as it is on any statement here: the
        // connection is discarded, and the next connection compares again.
        let digest: Option<Vec<u8>> = client
            .query_one(&self.digest_sql, &[])
            .await
            .map_err(trace::unavailable)?
            .try_get(0)
            .map_err(trace::unavailable)?;
        if digest.as_deref() == Some(&self.expected_digest[..]) {
            return client
                .batch_execute("ROLLBACK")
                .await
                .map_err(trace::unavailable);
        }
        #[cfg(test)]
        COMPARED_FULLY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let row = client
            .query_one(&self.sql, &[])
            .await
            .map_err(trace::unavailable)?;
        let body = row
            .try_get::<_, Option<String>>(0)
            .map_err(trace::unavailable)?
            .ok_or_else(|| trace::refuse("catalog_null"))?;
        if body.len() > BOUND {
            return Err(trace::refuse("catalog_oversize"));
        }
        let actual: Value =
            serde_json::from_str(&body).map_err(|_| trace::refuse("catalog_json"))?;
        if actual != self.expected {
            return Err(trace::refuse("catalog_mismatch"));
        }
        client
            .batch_execute("ROLLBACK")
            .await
            .map_err(trace::unavailable)
    }
}

/// `value` as PostgreSQL 16 prints a `jsonb` (`jsonb_out`): object keys in
/// jsonb's order (shorter first, then bytewise), `", "` between members and
/// elements, `": "` after a key, strings escaped as `escape_json` does, and
/// numbers in `serde_json`'s formatting, which is `numeric_out`'s for the
/// integers the pinned document holds (a float in a future pin would print
/// differently, so its digest would never match: every comparison would
/// fall back to the full document, which the native campaign's "a healthy
/// readiness run pulls no document" assertion catches). The catalog
/// statement builds its document with the jsonb functions and returns
/// `::text`, so this is the text the full comparison receives. Parsing it
/// back gives `value` again, which the unit test below holds for the pinned
/// document: a digest match therefore means the live document equals the
/// pinned value, not just some text of it.
pub(super) fn canonical(value: &Value) -> String {
    fn write(value: &Value, out: &mut String) {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Number(number) => out.push_str(&number.to_string()),
            Value::String(text) => escape(text, out),
            Value::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    write(item, out);
                }
                out.push(']');
            }
            Value::Object(members) => {
                let mut keys: Vec<&String> = members.keys().collect();
                keys.sort_by(|a, b| {
                    a.len()
                        .cmp(&b.len())
                        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
                });
                out.push('{');
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    escape(key, out);
                    out.push_str(": ");
                    write(&members[key], out);
                }
                out.push('}');
            }
        }
    }
    fn escape(text: &str, out: &mut String) {
        out.push('"');
        for c in text.chars() {
            match c {
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }
    let mut out = String::new();
    write(value, &mut out);
    out
}

/// Test executable only (APP-195): catalog comparisons run so far, on any
/// connection (role pools, readiness and the fixture's own checks).
#[cfg(test)]
static VERIFIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
pub(super) fn verifications() -> usize {
    VERIFIED.load(std::sync::atomic::Ordering::SeqCst)
}
/// Test executable only (APP-216): the comparisons so far that pulled the
/// whole catalog document (a digest that was not the pinned one).
#[cfg(test)]
static COMPARED_FULLY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
pub(super) fn compared_fully() -> usize {
    COMPARED_FULLY.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The safety of the digest path: a digest match means the database's
    /// text equals `canonical(expected)`, and that text parses back to
    /// `expected`. Holds for the pinned document, whose digest is what the
    /// database is compared with.
    #[test]
    fn the_pinned_document_round_trips_through_its_canonical_text() {
        let catalog = Catalog::embedded().unwrap();
        let text = canonical(&catalog.expected);
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, catalog.expected);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(text.as_bytes())),
            catalog.expected_digest
        );
        assert!(text.len() <= BOUND);
        assert!(catalog.digest_sql.ends_with(") AS catalog(body)"));
        assert!(catalog.digest_sql.contains(&catalog.sql));
    }

    /// The text form itself, on every JSON shape the catalog uses and on
    /// the characters `escape_json` treats specially. The native campaign
    /// compares the same values with the server's `jsonb` output.
    #[test]
    fn canonical_text_is_jsonb_output() {
        let value = json!({
            "b": {},
            "aa": [[], {"k": "v"}, {"two": 2, "one": 1, "三": 3}],
            "a": [1, -2, 0, 9007199254740993u64, true, false, null],
            "Z": "q\"\\/\n\r\t\u{8}\u{c}\u{1}\u{1f}\u{7f}é\u{20ac}",
        });
        assert_eq!(
            canonical(&value),
            "{\"Z\": \"q\\\"\\\\/\\n\\r\\t\\b\\f\\u0001\\u001f\u{7f}é\u{20ac}\", \
             \"a\": [1, -2, 0, 9007199254740993, true, false, null], \
             \"b\": {}, \
             \"aa\": [[], {\"k\": \"v\"}, {\"one\": 1, \"two\": 2, \"三\": 3}]}"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&canonical(&value)).unwrap(),
            value
        );
        assert_eq!(canonical(&json!([])), "[]");
        assert_eq!(canonical(&json!({})), "{}");
        assert_eq!(canonical(&json!("x")), "\"x\"");
    }
}
