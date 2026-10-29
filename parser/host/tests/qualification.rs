use parser_host::{Limits, Sandbox};
use serde_json::{json, Value};
use std::{
    io::{Cursor, Read, Write},
    path::PathBuf,
    sync::OnceLock,
    time::Duration,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn wasm() -> Vec<u8> {
    std::fs::read(root().join("target/wasm32-unknown-unknown/release/parser_guest.wasm"))
        .expect("build actual guest before tests")
}
fn sandbox() -> &'static Sandbox {
    static S: OnceLock<Sandbox> = OnceLock::new();
    S.get_or_init(|| Sandbox::new(&wasm()).expect("actual guest has no imports"))
}
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(root().join("fixtures").join(name)).unwrap()
}
fn config() -> Value {
    json!({"profile":"demo","format":"xlsx","sheet":"Inventory"})
}
fn run(bytes: &[u8], c: &Value) -> Value {
    sandbox().run(bytes, c, Limits::default()).unwrap().result
}
fn refused(bytes: &[u8], c: &Value, reason: &str) {
    let v = run(bytes, c);
    assert_eq!(v["status"], "refused", "{v}");
    assert_eq!(v["reason"], reason, "{v}");
    assert_eq!(v["no_revision_created"], true);
}
fn altered(base: &[u8], extra: (&str, &[u8])) -> Vec<u8> {
    let mut src = ZipArchive::new(Cursor::new(base)).unwrap();
    let mut out = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut f = src.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        out.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(&bytes).unwrap();
    }
    out.start_file(
        extra.0,
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .unwrap();
    out.write_all(extra.1).unwrap();
    out.finish().unwrap().into_inner()
}
#[test]
fn selected_sheet_literals_preserve_exact_numbers_and_coordinates() {
    let bytes = fixture("valid.xlsx");
    let report = sandbox().run(&bytes, &config(), Limits::default()).unwrap();
    let v = &report.result;
    assert_eq!(v["status"], "parsed", "{v}");
    assert_eq!(v["result"]["rows"], 2);
    let cells = v["result"]["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 6);
    assert_eq!(cells[0]["value"]["value"], "α & team");
    assert_eq!(cells[1]["value"]["value"], "9007199254740993");
    assert_eq!(cells[1]["original"], "9007199254740993");
    assert_eq!(cells[2]["value"]["value"], true);
    assert_eq!(cells[4]["value"]["coefficient"], "12345");
    assert_eq!(cells[4]["value"]["scale"], 2);
    assert_eq!(cells[5]["value"]["value"], "-125");
    assert_eq!(
        cells[4]["coordinate"],
        json!({"type":"xlsx","sheet":"Inventory","row":2,"column":2})
    );
    let other = run(
        &bytes,
        &json!({"profile":"demo","format":"xlsx","sheet":"Other"}),
    );
    assert_eq!(
        other["result"]["cells"][0]["value"]["value"],
        "Other sheet only"
    );
    assert_ne!(v["result"]["cells"], other["result"]["cells"]);
    assert!(report.memory_bytes < 2 * 1024 * 1024 * 1024);
    assert!(report.fuel_consumed > 0);
}
#[test]
fn csv_keeps_literal_text_logical_records_and_physical_spans() {
    let c = json!({"profile":"demo","format":"csv"});
    let v = run(&fixture("literal.csv"), &c);
    assert_eq!(v["status"], "parsed", "{v}");
    assert_eq!(v["result"]["rows"], 3);
    assert_eq!(v["result"]["cells"][4]["value"]["value"], "two\nlines");
    assert_eq!(
        v["result"]["cells"][4]["coordinate"],
        json!({"type":"csv","logical_record":2,"physical_line_start":2,"physical_line_end":3})
    );
    assert_eq!(
        v["result"]["cells"][5]["value"]["value"],
        "9007199254740993"
    );
    assert_eq!(v["result"]["cells"][6]["value"]["value"], "=SUM(A1)");
    refused(
        b"a,b\nx,y\n",
        &json!({"profile":"demo","format":"csv","limits":{"rows":1}}),
        "row_limit",
    );
    refused(
        b"a,b,c\n",
        &json!({"profile":"demo","format":"csv","limits":{"columns":2}}),
        "column_limit",
    );
    refused(&[255], &c, "csv_utf8_required");
    for (input, starts, ends) in [
        (b"a\rb\r".as_slice(), vec![1, 2], vec![1, 2]),
        (b"\r\na\r\n\r\nb".as_slice(), vec![2, 4], vec![2, 4]),
        (
            b"\"a\r\nb\",c\r\nx,y".as_slice(),
            vec![1, 1, 3, 3],
            vec![2, 2, 3, 3],
        ),
    ] {
        let v = run(input, &c);
        assert_eq!(v["status"], "parsed", "{v}");
        for ((cell, start), end) in v["result"]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .zip(starts)
            .zip(ends)
        {
            assert_eq!(cell["coordinate"]["physical_line_start"], start, "{v}");
            assert_eq!(cell["coordinate"]["physical_line_end"], end, "{v}");
        }
    }
}
#[test]
fn active_content_and_unbounded_metadata_are_refused_before_calamine() {
    for (name, reason) in [
        ("formula.xlsx", "formula_refused"),
        ("unselected-formula.xlsx", "formula_refused"),
        ("content-type-macro.xlsx", "macro_refused"),
        ("macro.xlsx", "macro_or_binary_refused"),
        ("external.xlsx", "external_link_refused"),
        ("declared-strings.xlsx", "shared_string_limit"),
        ("dimension.xlsx", "dimension_limit"),
        ("actual-coordinate.xlsx", "dimension_limit"),
        ("doctype.xlsx", "doctype_refused"),
        ("bad-numeric.xlsx", "number_exponent"),
    ] {
        refused(&fixture(name), &config(), reason);
    }
}
#[test]
fn resource_profiles_have_explicit_lower_only_controls() {
    let bytes = fixture("valid.xlsx");
    for (key, n, reason) in [
        ("input", 1, "input_limit"),
        ("entries", 2, "entry_limit"),
        ("expanded", 100, "expanded_limit"),
        ("strings", 1, "shared_string_limit"),
        ("cells", 3, "cell_limit"),
        ("rows", 1, "dimension_limit"),
        ("columns", 1, "dimension_limit"),
        ("sheets", 1, "sheet_limit"),
        ("output", 100, "output_limit"),
        ("text", 2, "xml_token_limit"),
    ] {
        let mut c = config();
        c["limits"] = json!({key:n});
        refused(&bytes, &c, reason);
    }
    let mut c = config();
    c["limits"] = json!({"input":6*1024*1024});
    refused(&bytes, &c, "limit_override_may_only_reduce");
    c = json!({"profile":"platform","format":"xlsx","sheet":"Inventory"});
    assert_eq!(run(&bytes, &c)["status"], "parsed");
    let at = vec![b'a'; 10 * 1024 * 1024];
    let over = vec![b'a'; 10 * 1024 * 1024 + 1];
    let c = json!({"profile":"demo","format":"csv"});
    assert_eq!(
        sandbox().run(&at, &c, Limits::default()).unwrap().result["reason"],
        "text_limit"
    );
    assert!(sandbox()
        .run(&over, &c, Limits::default())
        .unwrap_err()
        .to_string()
        .contains("input_limit"));
}
#[test]
fn archive_paths_counts_and_actual_expansion_are_bounded() {
    refused(
        &altered(&fixture("valid.xlsx"), ("../escape.xml", b"<x/>")),
        &config(),
        "archive_path",
    );
    let payload = vec![b'x'; 8192];
    let mut bytes = altered(&fixture("valid.xlsx"), ("padding.dat", &payload));
    // Lie in the directory about the padding's expanded bytes; actual inflation must still stop.
    let p = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == b"PK\x01\x02")
        .map(|(i, _)| i)
        .next_back()
        .unwrap();
    let local = u32::from_le_bytes(bytes[p + 42..p + 46].try_into().unwrap()) as usize;
    bytes[p + 24..p + 28].copy_from_slice(&1u32.to_le_bytes());
    bytes[local + 22..local + 26].copy_from_slice(&1u32.to_le_bytes());
    let mut c = config();
    c["limits"] = json!({"expanded":4096});
    let v = run(&bytes, &c);
    assert_eq!(v["status"], "refused", "{v}");
    assert!(
        matches!(
            v["reason"].as_str(),
            Some("expanded_limit" | "archive_expansion_or_crc" | "archive_size_mismatch")
        ),
        "{v}"
    );
}
#[test]
fn demo_actual_compressed_envelope_and_fifty_mib_refusal() {
    let base = fixture("valid.xlsx");
    let mut source = ZipArchive::new(Cursor::new(&base)).unwrap();
    let mut base_expanded = 0usize;
    let mut out = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..source.len() {
        let mut file = source.by_index(i).unwrap();
        let name = file.name().unwrap().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        base_expanded += data.len();
        out.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(&data).unwrap();
    }
    out.start_file(
        "bomb.dat",
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .unwrap();
    let block = vec![b'x'; 1024 * 1024];
    for _ in 0..51 {
        out.write_all(&block).unwrap();
    }
    out.start_file(
        "padding.dat",
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
    .unwrap();
    for _ in 0..4 {
        out.write_all(&block).unwrap();
    }
    let bytes = out.finish().unwrap().into_inner();
    assert!(bytes.len() > 4 * 1024 * 1024 && bytes.len() < 5 * 1024 * 1024);
    let report = sandbox().run(&bytes, &config(), Limits::default()).unwrap();
    assert_eq!(report.result["reason"], "expanded_limit");
    assert!(report.memory_bytes < 128 * 1024 * 1024);
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
    println!(
        "DEMO_BOMB compressed_bytes={} expanded_bytes={} report={}",
        bytes.len(),
        55 * 1024 * 1024 + base_expanded,
        serde_json::to_string(&report).unwrap()
    );
}
fn control(parse: &str) -> Vec<u8> {
    format!(r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) i32.const 0) (func (export "parse") (param i32 i32 i32 i32) (result i32) {parse}) (func (export "result_ptr") (result i32) i32.const 0) (func (export "result_len") (result i32) i32.const 0))"#).into_bytes()
}
#[test]
fn runtime_refuses_system_imports() {
    for interface in ["fd_read", "sock_open", "environ_get", "credentials"] {
        let wat = format!(r#"(module (import "wasi_snapshot_preview1" "{interface}" (func)))"#);
        assert!(Sandbox::new(wat.as_bytes())
            .err()
            .unwrap()
            .to_string()
            .contains("forbidden_import"));
    }
}
#[test]
fn actual_guest_fuel_and_deadline_failures_isolated_then_recovery() {
    let low = Limits {
        fuel: 1,
        ..Limits::default()
    };
    let report = sandbox()
        .run(&fixture("valid.xlsx"), &config(), low)
        .unwrap();
    assert_eq!(report.result["reason"], "fuel_exhausted");
    let csv = b"a\n".repeat(100_000);
    let report = sandbox()
        .run(
            &csv,
            &json!({"profile":"demo","format":"csv"}),
            Limits {
                timeout: Duration::from_millis(1),
                ..Limits::default()
            },
        )
        .unwrap();
    assert_eq!(report.result["reason"], "time_limit");
    assert!(report.elapsed_ms < 2000);
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
}
#[test]
fn infinite_loop_deadline_and_memory_growth_trap_without_killing_host() {
    let spin = Sandbox::new(&control("(loop $again br $again) i32.const 0")).unwrap();
    let limits = Limits {
        timeout: Duration::from_millis(20),
        ..Limits::default()
    };
    let report = spin.run(b"", &config(), limits).unwrap();
    assert_eq!(report.result["reason"], "time_limit");
    assert!(
        report.elapsed_ms < 2000,
        "timer did not interrupt: {}ms",
        report.elapsed_ms
    );
    let grow = Sandbox::new(&control("i32.const 1000 memory.grow drop i32.const 0")).unwrap();
    let report = grow
        .run(
            b"",
            &config(),
            Limits {
                memory_bytes: 2 * 1024 * 1024,
                ..Limits::default()
            },
        )
        .unwrap();
    assert_eq!(report.result["reason"], "memory_limit");
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
}
#[test]
fn deterministic_mutation_fuzz_corpus_stays_inside_wasm_and_recovers() {
    let seeds = [fixture("literal.csv"), fixture("valid.xlsx")];
    let mut rng = 0x7a61_4c32u32;
    for (seed_no, seed) in seeds.iter().enumerate() {
        for iteration in 0..128 {
            let mut bytes = seed.clone();
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            match iteration % 4 {
                0 => {
                    let p = rng as usize % bytes.len();
                    bytes[p] ^= (rng >> 8) as u8 | 1;
                }
                1 => bytes.truncate(rng as usize % bytes.len()),
                2 => {
                    let p = rng as usize % bytes.len();
                    bytes.splice(p..p, [0, 255, 60, 62]);
                }
                _ => {
                    let p = rng as usize % bytes.len();
                    let end = (p + 16).min(bytes.len());
                    bytes[p..end].fill(0);
                }
            }
            let c = if seed_no == 0 {
                json!({"profile":"demo","format":"csv"})
            } else {
                config()
            };
            let limits = Limits {
                fuel: 20_000_000,
                timeout: Duration::from_secs(2),
                memory_bytes: 64 * 1024 * 1024,
                ..Limits::default()
            };
            let report = sandbox().run(&bytes, &c, limits).unwrap();
            assert!(report.result["status"] == "parsed" || report.result["status"] == "refused");
            assert!(report.memory_bytes <= 64 * 1024 * 1024);
        }
    }
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
    println!("FUZZ 256 deterministic mutation cases; seed=0x7a614c32; two actual parser formats; memory/fuel/wall caps active");
}

#[test]
fn archive_advisory_regressions_are_refused_before_dependency_read() {
    let base = fixture("valid.xlsx");
    let p = base.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    let local = u32::from_le_bytes(base[p + 42..p + 46].try_into().unwrap()) as usize;
    // GHSA-m665: the local alias must never differ from the inspected directory name.
    let mut b = base.clone();
    b[local + 30] = b'X';
    refused(&b, &config(), "archive_name_mismatch");
    let mut b = base.clone();
    b[local + 14] ^= 1;
    refused(&b, &config(), "archive_header_mismatch");
    // Reject unsupported methods and encryption before any legacy/AES decoder exists.
    for method in [1u16, 2, 3, 4, 5, 6, 9, 99] {
        let mut b = base.clone();
        b[p + 10..p + 12].copy_from_slice(&method.to_le_bytes());
        refused(&b, &config(), "archive_method_or_encryption");
    }
    let mut b = base.clone();
    b[p + 8] |= 1;
    refused(&b, &config(), "archive_method_or_encryption");
    // SUID/SGID, sticky bits, symlinks, devices: no extraction API and no such metadata.
    for mode in [0o104755u32, 0o102755, 0o101755, 0o120777, 0o020600] {
        let mut b = base.clone();
        b[p + 38..p + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        refused(&b, &config(), "archive_unsafe_mode");
    }
    // Extra fields can provide Unicode aliases or ZIP64 sizes; this subset refuses all.
    let mut b = base.clone();
    b[p + 30..p + 32].copy_from_slice(&4u16.to_le_bytes());
    refused(&b, &config(), "archive_extra_unsupported");
    // GHSA-wjr6 and GHSA-gmfj: no earlier footer fallback, missing footer or repaired offsets.
    let mut b = base.clone();
    b.extend_from_slice(&base);
    refused(&b, &config(), "archive_ambiguous_footer");
    refused(&base[..base.len() - 22], &config(), "archive_footer");
    let mut b = base.clone();
    let end = b.len() - 22;
    b[end + 16..end + 20].copy_from_slice(&0u32.to_le_bytes());
    refused(&b, &config(), "archive_directory");
    assert_eq!(run(&base, &config())["status"], "parsed");
}
#[test]
fn actual_guest_memory_and_output_failure_are_isolated() {
    let input = fixture("valid.xlsx");
    let limits = Limits {
        memory_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let report = sandbox().run(&input, &config(), limits).unwrap();
    assert_eq!(report.result["reason"], "memory_limit");
    let limits = Limits {
        max_output_bytes: 1,
        ..Limits::default()
    };
    let report = sandbox().run(&input, &config(), limits).unwrap();
    assert_eq!(report.result["reason"], "output_limit");
    assert_eq!(run(&input, &config())["status"], "parsed");
}
#[test]
fn metadata_counts_and_selection_need_actual_validated_content() {
    refused(
        &fixture("actual-strings.xlsx"),
        &json!({"profile":"demo","format":"xlsx","sheet":"Inventory","limits":{"strings":2}}),
        "shared_string_limit",
    );
    refused(&fixture("duplicate-cell.xlsx"), &config(), "cell_duplicate");
    refused(&fixture("deep.xml.xlsx"), &config(), "xml_depth_limit");
    refused(
        &fixture("valid.xlsx"),
        &json!({"profile":"demo","format":"xlsx"}),
        "sheet_selection_required",
    );
    refused(
        &fixture("valid.xlsx"),
        &json!({"profile":"demo","format":"xlsx","sheet":"missing"}),
        "sheet_not_found",
    );
}

#[test]
fn host_limit_overrides_cannot_expand_the_admitted_envelope() {
    let max = Limits::default();
    let cases = [
        Limits {
            memory_bytes: max.memory_bytes + 1,
            ..max.clone()
        },
        Limits {
            timeout: max.timeout + Duration::from_nanos(1),
            ..max.clone()
        },
        Limits {
            fuel: max.fuel + 1,
            ..max.clone()
        },
        Limits {
            max_input_bytes: max.max_input_bytes + 1,
            ..max.clone()
        },
        Limits {
            max_output_bytes: max.max_output_bytes + 1,
            ..max.clone()
        },
    ];
    for limits in cases {
        let error = sandbox()
            .run(&fixture("valid.xlsx"), &config(), limits)
            .unwrap_err();
        assert_eq!(error.to_string(), "runtime_limit_may_not_increase");
    }
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
}

#[test]
fn review_zero_exponents_are_canonical_in_actual_xlsx() {
    let v = run(&fixture("zero-exponents.xlsx"), &config());
    assert_eq!(v["status"], "parsed", "{v}");
    for index in [4, 5] {
        assert_eq!(v["result"]["cells"][index]["value"]["type"], "integer");
        assert_eq!(v["result"]["cells"][index]["value"]["value"], "0");
    }
    assert_eq!(v["result"]["cells"][4]["original"], "0e3");
    assert_eq!(v["result"]["cells"][5]["original"], "-0e3");
}
#[test]
fn review_duplicate_cell_values_are_refused_before_literal_selection() {
    refused(
        &fixture("duplicate-values.xlsx"),
        &config(),
        "cell_value_duplicate",
    );
    assert_eq!(run(&fixture("valid.xlsx"), &config())["status"], "parsed");
}
