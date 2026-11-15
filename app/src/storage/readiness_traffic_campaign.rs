//! APP-216 part 1: the bytes the database transmits for one readiness run,
//! read from the local Docker fixture's network interface, with the number
//! of catalog comparisons that ran and how many of them pulled the whole
//! document. The same probe measured the code before and after ADR-004
//! amendment A6 (the catalog digest); both reports are in
//! `evidence/APP-216/`. Local evidence only: the beta's figure is the RDS
//! `NetworkTransmitThroughput` metric after the release.
use super::{
    catalog,
    pool::Stores,
    qualification::{docker, Fixture, IMAGE},
};
use serde_json::json;
use std::time::Instant;

/// `(rx_bytes, tx_bytes)` of the container's `eth0`, from `/proc/net/dev`:
/// what the database received from and transmitted to the service.
fn interface_bytes(container: &str) -> (u64, u64) {
    let text = docker(
        &["exec", container, "cat", "/proc/net/dev"],
        b"",
        "interface counters",
    );
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        if name.trim() != "eth0" {
            continue;
        }
        let fields: Vec<u64> = rest
            .split_whitespace()
            .map(|field| field.parse().expect("a /proc/net/dev counter"))
            .collect();
        // receive: bytes packets errs drop fifo frame compressed multicast;
        // then transmit: bytes ...
        return (fields[0], fields[8]);
    }
    panic!("no eth0 in /proc/net/dev:\n{text}");
}

#[tokio::test]
#[ignore]
async fn native_readiness_traffic_campaign() {
    const RUNS: u64 = 20;
    let fixture = Fixture::new();
    let stores = Stores::new(fixture.settings()).unwrap();
    // The first run creates the four connections (TLS, admission); the
    // measured runs reuse them, as the beta's probe does every 15 s.
    stores.readiness().await.unwrap();
    let embedded = catalog::Catalog::embedded().unwrap();
    let (full_sql, digest_sql) = embedded.statements();
    let operator = fixture.operator().await;
    operator
        .client
        .batch_execute("BEGIN READ ONLY; SET LOCAL search_path=pg_catalog,pg_temp")
        .await
        .unwrap();
    let body: String = operator
        .client
        .query_one(full_sql, &[])
        .await
        .unwrap()
        .get(0);
    let digest: Vec<u8> = operator
        .client
        .query_one(digest_sql, &[])
        .await
        .unwrap()
        .get(0);
    operator.client.batch_execute("ROLLBACK").await.unwrap();
    let (rx_before, tx_before) = interface_bytes(&fixture.name);
    let verified_before = catalog::verifications();
    let full_before = catalog::compared_fully();
    let started = Instant::now();
    for _ in 0..RUNS {
        stores.readiness().await.unwrap();
    }
    let elapsed = started.elapsed();
    let (rx_after, tx_after) = interface_bytes(&fixture.name);
    let comparisons = catalog::verifications() - verified_before;
    let full = catalog::compared_fully() - full_before;
    stores.close().await.unwrap();
    let report = json!({
        "schema_version": 1,
        "scope": "APP-216 part 1: database bytes per readiness run on the local Docker fixture",
        "postgres_image": IMAGE,
        "server_version": fixture.server_version,
        "runs": RUNS,
        "catalog_document_octets": body.len(),
        "catalog_digest_octets": digest.len(),
        "comparisons": comparisons,
        "comparisons_pulling_the_document": full,
        "db_transmit_bytes_total": tx_after - tx_before,
        "db_transmit_bytes_per_run": (tx_after - tx_before) / RUNS,
        "db_receive_bytes_total": rx_after - rx_before,
        "db_receive_bytes_per_run": (rx_after - rx_before) / RUNS,
        "db_transmit_bytes_per_second_at_15s": (tx_after - tx_before) / RUNS / 15,
        "db_transmit_bytes_per_day_at_15s": (tx_after - tx_before) / RUNS * 5760,
        "elapsed_ms_per_run": elapsed.as_millis() as u64 / RUNS,
        "limits": [
            "Bytes are the container interface's counters: TLS records and protocol framing included, four role pools per run.",
            "A local Docker PostgreSQL on a shared macOS host, not the beta's RDS instance; the beta's figure is its NetworkTransmitThroughput metric.",
        ],
    });
    println!("APP216 report: {report}");
    if let Some(path) = std::env::var_os("GENUI_READINESS_TRAFFIC_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    assert_eq!(
        comparisons,
        4 * RUNS as usize,
        "readiness compares every pool on every run"
    );
}
