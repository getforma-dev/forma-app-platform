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
/// The guest bytes under test: `PARSER_GUEST_WASM` names other bytes, such as
/// the guest extracted from a built image (ADR-009 D9.1); by default the
/// locally built guest.
fn wasm_path() -> PathBuf {
    std::env::var_os("PARSER_GUEST_WASM").map_or_else(
        || root().join("target/wasm32-unknown-unknown/release/parser_guest.wasm"),
        PathBuf::from,
    )
}
fn wasm() -> Vec<u8> {
    std::fs::read(wasm_path()).expect("build actual guest before tests")
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

#[test]
fn qualified_guest_bytes_are_named_in_the_log() {
    let bytes = wasm();
    println!(
        "QUALIFIED_WASM {} bytes={} path={}",
        parser_host::sha(&bytes),
        bytes.len(),
        wasm_path().display()
    );
    assert!(sandbox()
        .run(
            b"a\n",
            &json!({"profile":"demo","format":"csv"}),
            Limits::default()
        )
        .is_ok());
}

fn replace_entry(base: &[u8], wanted: &str, replacement: &[u8]) -> Vec<u8> {
    let mut src = ZipArchive::new(Cursor::new(base)).unwrap();
    let mut out = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut f = src.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        out.start_file(
            &name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(if name == wanted { replacement } else { &bytes })
            .unwrap();
    }
    out.finish().unwrap().into_inner()
}

/// Produce ordinary streaming ZIP descriptors from a validated fixture without
/// changing compressed payloads. The CRC and both sizes come from its directory.
fn with_descriptors(base: &[u8], signed: bool) -> (Vec<u8>, Vec<usize>) {
    let u16at = |p| u16::from_le_bytes(base[p..p + 2].try_into().unwrap()) as usize;
    let u32at = |p| u32::from_le_bytes(base[p..p + 4].try_into().unwrap()) as usize;
    let end = base.len() - 22;
    let mut at = u32at(end + 16);
    let mut out = Vec::new();
    let mut directory = Vec::new();
    let mut descriptors = Vec::new();
    for _ in 0..u16at(end + 10) {
        let local = u32at(at + 42);
        let n = u16at(at + 28);
        assert_eq!(u16at(at + 30), 0);
        assert_eq!(u16at(at + 32), 0);
        let mut header = base[local..local + 30 + n].to_vec();
        let flags = (u16at(at + 8) as u16 | 8).to_le_bytes();
        header[6..8].copy_from_slice(&flags);
        header[14..26].fill(0);
        let offset = out.len() as u32;
        out.extend_from_slice(&header);
        let size = u32at(at + 20);
        out.extend_from_slice(&base[local + 30 + n..local + 30 + n + size]);
        descriptors.push(out.len());
        if signed {
            out.extend_from_slice(&0x08074b50u32.to_le_bytes());
        }
        out.extend_from_slice(&base[at + 16..at + 28]);
        let mut central = base[at..at + 46 + n].to_vec();
        central[8..10].copy_from_slice(&flags);
        central[42..46].copy_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(&central);
        at += 46 + n;
    }
    let central = out.len() as u32;
    let size = directory.len() as u32;
    out.extend_from_slice(&directory);
    let mut footer = base[end..].to_vec();
    footer[12..16].copy_from_slice(&size.to_le_bytes());
    footer[16..20].copy_from_slice(&central.to_le_bytes());
    out.extend_from_slice(&footer);
    (out, descriptors)
}

#[test]
fn view_pages_wide_ragged_csv_and_keeps_source_coordinates_without_truncation() {
    let full = (1..=153)
        .map(|n| format!("column{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let input = format!("{full}\r\nshort,row\r\n{full}\r\n");
    let c = json!({"profile":"view","format":"csv","window":{"row_offset":1,"column_offset":128}});
    let value = run(input.as_bytes(), &c);
    assert_eq!(value["status"], "parsed", "{value}");
    let result = &value["result"];
    assert_eq!(result["row_count"], 3);
    assert_eq!(result["column_count"], 153);
    assert_eq!(result["row_offset"], 1);
    assert_eq!(result["column_offset"], 128);
    assert_eq!(result["page_rows"], 50);
    assert_eq!(result["page_columns"], 32);
    let cells = result["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 25);
    assert_eq!(cells[0]["column"], 129);
    assert_eq!(cells[0]["coordinate"]["logical_record"], 3);
    assert_eq!(cells[0]["coordinate"]["physical_line_start"], 3);
    assert_eq!(cells[0]["original"], "column129");
    assert_eq!(cells.last().unwrap()["original"], "column153");
    assert_eq!(result["formula_count"], 0);
    assert_eq!(result["formula_cells"], json!([]));
    refused(
        input.as_bytes(),
        &json!({"profile":"demo","format":"csv"}),
        "column_limit",
    );
    refused(
        b"a,b",
        &json!({"profile":"demo","format":"csv","window":{"row_offset":0,"column_offset":0}}),
        "window_requires_view",
    );
    refused(
        b"a,b",
        &json!({"profile":"platform","format":"csv","window":{"row_offset":0,"column_offset":0}}),
        "window_requires_view",
    );
    refused(
        b"a,b",
        &json!({"profile":"view","format":"csv","window":{"row_offset":1048576,"column_offset":0}}),
        "window_invalid",
    );
    refused(
        b"a,b",
        &json!({"profile":"view","format":"csv","limits":{"cells":1}}),
        "cell_limit",
    );
    refused(
        &vec![b'a'; 65_537],
        &json!({"profile":"view","format":"csv"}),
        "text_limit",
    );
    assert_eq!(
        run(
            &vec![b'a'; 65_536],
            &json!({"profile":"view","format":"csv"})
        )["status"],
        "parsed"
    );
    let unicode = "😀".repeat(65_536) + ",last\n";
    let exact = run(
        unicode.as_bytes(),
        &json!({"profile":"view","format":"csv"}),
    );
    assert_eq!(exact["status"], "parsed");
    assert_eq!(
        exact["result"]["cells"][0]["original"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        65_536
    );
    assert_eq!(exact["result"]["cells"][1]["original"], "last");
}

#[test]
fn view_streams_more_than_a_hundred_thousand_csv_cells_with_bounded_page_output() {
    let row = (0..153).map(|_| "001").collect::<Vec<_>>().join(",") + "\n";
    let input = row.repeat(1000);
    let report=sandbox().run(input.as_bytes(), &json!({"profile":"view","format":"csv","window":{"row_offset":950,"column_offset":128}}), Limits::default()).unwrap();
    assert_eq!(report.result["status"], "parsed", "{}", report.result);
    assert_eq!(report.result["result"]["row_count"], 1000);
    assert_eq!(report.result["result"]["column_count"], 153);
    assert_eq!(
        report.result["result"]["cells"].as_array().unwrap().len(),
        1250
    );
    let bytes = serde_json::to_vec(&report.result).unwrap().len();
    assert!(bytes < 512 * 1024);
    assert_eq!(
        report.resource_limits["guest_linear_memory_bytes"],
        512 * 1024 * 1024
    );
    assert_eq!(report.resource_limits["output_bytes"], 8 * 1024 * 1024);
    println!("VIEW_CSV cells=153000 input_bytes={} output_bytes={bytes} elapsed_ms={} memory_bytes={} fuel={}",input.len(),report.elapsed_ms,report.memory_bytes,report.fuel_consumed);
}

#[test]
fn view_reads_cached_formulas_and_marks_missing_results_without_evaluation() {
    let sheet=br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:AZ52"/><sheetData><row r="1"><c r="A1"><f>1+1</f><v>2.00</v></c><c r="B1" t="str"><f>"cached"</f><v>A &amp; B</v></c><c r="C1" t="b"><f>TRUE()</f><v>1</v></c><c r="D1"><f>NOW()</f><v/></c><c r="E1" t="str"><f>""</f><v/></c></row><row r="52"><c r="AZ52"><f>1+2</f></c></row></sheetData></worksheet>"#;
    let bytes = replace_entry(&fixture("valid.xlsx"), "xl/worksheets/sheet1.xml", sheet);
    let c = json!({"profile":"view","format":"xlsx","sheet":"Inventory"});
    let value = run(&bytes, &c);
    assert_eq!(value["status"], "parsed", "{value}");
    let result = &value["result"];
    assert_eq!(result["row_count"], 52);
    assert_eq!(result["column_count"], 52);
    assert_eq!(result["formula_count"], 6);
    assert_eq!(result["missing_formula_results"], 2);
    assert_eq!(
        result["formula_cells"],
        json!([{"row":1,"column":1,"cached":true},{"row":1,"column":2,"cached":true},{"row":1,"column":3,"cached":true},{"row":1,"column":4,"cached":false},{"row":1,"column":5,"cached":true}])
    );
    let cells = result["cells"].as_array().unwrap();
    assert_eq!(cells[0]["original"], "2.00");
    assert_eq!(cells[1]["original"], "A & B");
    assert_eq!(cells[2]["original"], true);
    assert!(cells
        .iter()
        .any(|cell| cell["coordinate"]["column"] == 5 && cell["original"] == ""));
    assert!(cells
        .iter()
        .any(|cell| cell["coordinate"]["column"] == 4 && cell["original"].is_null()));
    let page = run(
        &bytes,
        &json!({"profile":"view","format":"xlsx","sheet":"Inventory","window":{"row_offset":50,"column_offset":32}}),
    );
    assert_eq!(page["status"], "parsed", "{page}");
    assert_eq!(page["result"]["cells"].as_array().unwrap().len(), 1);
    assert_eq!(
        page["result"]["formula_cells"],
        json!([{"row":52,"column":52,"cached":false}])
    );
    assert_eq!(
        page["result"]["cells"][0]["coordinate"],
        json!({"type":"xlsx","sheet":"Inventory","row":52,"column":52})
    );
    let discovery = run(&bytes, &json!({"profile":"view","format":"xlsx"}));
    assert_eq!(discovery["reason"], "sheet_selection_required");
    assert_eq!(discovery["available_sheets"], json!(["Inventory", "Other"]));
    refused(&fixture("formula.xlsx"), &config(), "formula_refused");
}

#[test]
fn view_accepts_only_validated_signed_or_unsigned_zip_descriptors() {
    let c = json!({"profile":"view","format":"xlsx","sheet":"Inventory"});
    for signed in [false, true] {
        let (bytes, positions) = with_descriptors(&fixture("valid.xlsx"), signed);
        assert_eq!(run(&bytes, &c)["status"], "parsed");
        refused(&bytes, &config(), "archive_method_or_encryption");
        for delta in [0, 4, 8] {
            let mut bad = bytes.clone();
            bad[positions[0] + if signed { 4 } else { 0 } + delta] ^= 1;
            refused(&bad, &c, "archive_descriptor_mismatch");
        }
        let mut encrypted = bytes.clone();
        let central = u32::from_le_bytes(
            encrypted[encrypted.len() - 6..encrypted.len() - 2]
                .try_into()
                .unwrap(),
        ) as usize;
        encrypted[6] |= 1;
        encrypted[central + 8] |= 1;
        refused(&encrypted, &c, "archive_method_or_encryption");
        let mut truncated = bytes.clone();
        truncated.truncate(positions[0] + 5);
        assert_eq!(run(&truncated, &c)["status"], "refused");
        let mut span = bytes.clone();
        let expanded = u32::from_le_bytes(span[central + 24..central + 28].try_into().unwrap());
        let compressed = u32::from_le_bytes(span[central + 20..central + 24].try_into().unwrap());
        span[central + 20..central + 24].copy_from_slice(&(compressed + 1).to_le_bytes());
        let descriptor = positions[0] + usize::from(signed) * 4;
        span[descriptor + 4..descriptor + 8].copy_from_slice(&(compressed + 1).to_le_bytes());
        span[descriptor + 8..descriptor + 12].copy_from_slice(&expanded.to_le_bytes());
        assert_eq!(run(&span, &c)["status"], "refused");
        let mut flags = bytes.clone();
        flags[6] ^= 1;
        refused(&flags, &c, "archive_header_mismatch");
        let mut name = bytes.clone();
        name[30] ^= 1;
        refused(&name, &c, "archive_name_mismatch");
    }
    for (name, reason) in [
        ("macro.xlsx", "macro_or_binary_refused"),
        ("content-type-macro.xlsx", "macro_refused"),
        ("external.xlsx", "external_link_refused"),
        ("doctype.xlsx", "doctype_refused"),
        ("duplicate-cell.xlsx", "cell_duplicate"),
        ("duplicate-values.xlsx", "cell_value_duplicate"),
    ] {
        refused(&fixture(name), &c, reason);
        refused(
            &fixture(name),
            &json!({"profile":"view","format":"xlsx","sheet":"Other","window":{"row_offset":1000,"column_offset":128}}),
            reason,
        );
    }
}

#[test]
fn view_streams_more_than_a_hundred_thousand_xlsx_cells_with_bounded_page_output() {
    let columns = (1..=153)
        .map(|mut column| {
            let mut name = String::new();
            while column > 0 {
                column -= 1;
                name.insert(0, (b'A' + (column % 26) as u8) as char);
                column /= 26;
            }
            name
        })
        .collect::<Vec<_>>();
    let mut xml=String::from("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>");
    for row in 1..=1000 {
        xml.push_str(&format!("<row r=\"{row}\">"));
        for column in &columns {
            xml.push_str(&format!("<c r=\"{column}{row}\"><v>1.00</v></c>"));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    let input = replace_entry(
        &fixture("valid.xlsx"),
        "xl/worksheets/sheet1.xml",
        xml.as_bytes(),
    );
    let report=sandbox().run(&input, &json!({"profile":"view","format":"xlsx","sheet":"Inventory","window":{"row_offset":950,"column_offset":128}}), Limits::default()).unwrap();
    assert_eq!(report.result["status"], "parsed", "{}", report.result);
    assert_eq!(report.result["result"]["row_count"], 1000);
    assert_eq!(report.result["result"]["column_count"], 153);
    assert_eq!(
        report.result["result"]["cells"].as_array().unwrap().len(),
        1250
    );
    assert_eq!(report.result["result"]["cells"][0]["original"], "1.00");
    let bytes = serde_json::to_vec(&report.result).unwrap().len();
    assert!(bytes < 512 * 1024);
    println!("VIEW_XLSX cells=153000 input_bytes={} output_bytes={bytes} elapsed_ms={} memory_bytes={} fuel={}",input.len(),report.elapsed_ms,report.memory_bytes,report.fuel_consumed);
}

#[test]
fn view_refuses_cached_string_cdata_and_reordered_formula_before_dependency_loss() {
    let c = json!({"profile":"view","format":"xlsx","sheet":"Inventory"});
    for (cell, reason) in [
        (
            "<c r=\"A1\" t=\"str\"><f>\"hello\"</f><v><![CDATA[hello]]></v></c>",
            "numeric_cdata_refused",
        ),
        (
            "<c r=\"A1\"><v>2</v><f>1+1</f></c>",
            "cell_formula_structure",
        ),
    ] {
        let xml=format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\">{cell}</row></sheetData></worksheet>");
        refused(
            &replace_entry(
                &fixture("valid.xlsx"),
                "xl/worksheets/sheet1.xml",
                xml.as_bytes(),
            ),
            &c,
            reason,
        );
    }
}
