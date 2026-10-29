//! `parser-host --job` through the actual executable: one framed job per
//! process, spawned with an empty environment and only its pipes (ADR-009 D9.1).
//!
//! `PARSER_GUEST_WASM` and `PARSER_HOST_BIN` select other bytes under test,
//! such as the guest and host extracted from a built image; by default the
//! locally built guest and this package's binary are used.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn wasm_path() -> PathBuf {
    std::env::var_os("PARSER_GUEST_WASM").map_or_else(
        || root().join("target/wasm32-unknown-unknown/release/parser_guest.wasm"),
        PathBuf::from,
    )
}
fn host_path() -> PathBuf {
    std::env::var_os("PARSER_HOST_BIN").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_parser-host")),
        PathBuf::from,
    )
}
fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn wasm_hex() -> String {
    hex(&std::fs::read(wasm_path()).expect("build the actual guest before tests"))
}
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(root().join("fixtures").join(name)).unwrap()
}
fn frame(config: &[u8], input: &[u8]) -> Vec<u8> {
    let mut bytes = (config.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(config);
    bytes.extend_from_slice(input);
    bytes
}
struct Job {
    code: i32,
    stdout: Vec<u8>,
    stderr: String,
}
impl Job {
    fn document(&self) -> Value {
        serde_json::from_slice(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not one JSON document ({e}); stderr={}",
                self.stderr
            )
        })
    }
    fn report(&self) -> Value {
        let lines: Vec<_> = self.stderr.lines().collect();
        assert_eq!(lines.len(), 1, "stderr must be one line: {}", self.stderr);
        serde_json::from_str(lines[0]).unwrap()
    }
}
fn run_with(args: &[&str], stdin: &[u8]) -> Job {
    let mut child = Command::new(host_path())
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let input = stdin.to_vec();
    // The host may stop reading at its input cap; a closed pipe is not a failure.
    let writer = std::thread::spawn(move || {
        let _ = pipe.write_all(&input);
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    Job {
        code: output.status.code().expect("exited, not signalled"),
        stdout: output.stdout,
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}
fn run(stdin: &[u8]) -> Job {
    let wasm = wasm_path();
    let digest = wasm_hex();
    run_with(
        &[
            "--job",
            "--wasm",
            wasm.to_str().unwrap(),
            "--wasm-sha256",
            &digest,
        ],
        stdin,
    )
}
fn config(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap()
}
fn assert_refused(job: &Job, reason: &str) {
    assert_eq!(job.code, 2, "stderr={}", job.stderr);
    let document = job.document();
    assert_eq!(document["status"], "refused", "{document}");
    assert_eq!(document["reason"], reason, "{document}");
    assert_eq!(document["no_revision_created"], true);
    let report = job.report();
    assert_eq!(report["status"], "refused");
    assert_eq!(report["reason"], reason);
}

#[test]
fn job_mode_reads_framed_config_and_input() {
    let input = fixture("valid.xlsx");
    let c = config(json!({"profile":"demo","format":"xlsx","sheet":"Inventory"}));
    let job = run(&frame(&c, &input));
    assert_eq!(job.code, 0, "stderr={}", job.stderr);
    let document = job.document();
    assert_eq!(document["status"], "parsed", "{document}");
    assert_eq!(document["result"]["selected_sheet"], "Inventory");
    assert_eq!(document["result"]["cells"].as_array().unwrap().len(), 6);
    assert_eq!(
        document["result"]["cells"][1]["value"]["value"],
        "9007199254740993"
    );
    let report = job.report();
    assert_eq!(report["status"], "parsed");
    assert_eq!(report["input_sha256"], format!("sha256:{}", hex(&input)));
    assert_eq!(report["wasm_sha256"], format!("sha256:{}", wasm_hex()));
    assert_eq!(report["interface_imports"], json!([]));
    assert_eq!(report["resource_limits"]["wall_clock_ms"], 120_000);
    assert_eq!(report["resource_limits"]["input_bytes"], 5 * 1024 * 1024);
    assert!(
        report.get("result").is_none(),
        "the document travels on stdout only"
    );

    let csv = fixture("literal.csv");
    let job = run(&frame(
        &config(json!({"profile":"demo","format":"csv"})),
        &csv,
    ));
    assert_eq!(job.code, 0, "stderr={}", job.stderr);
    let document = job.document();
    assert_eq!(document["result"]["rows"], 3);
    assert_eq!(
        document["result"]["cells"][4]["value"]["value"],
        "two\nlines"
    );
    assert_eq!(
        job.report()["input_sha256"],
        format!("sha256:{}", hex(&csv))
    );
}

#[test]
fn sheet_selection_required_lists_sheets_after_preflight() {
    let job = run(&frame(
        &config(json!({"profile":"demo","format":"xlsx"})),
        &fixture("valid.xlsx"),
    ));
    assert_refused(&job, "sheet_selection_required");
    assert_eq!(
        job.document()["available_sheets"],
        json!(["Inventory", "Other"])
    );
    // The workbook-wide preflight runs first: a hostile workbook is refused for
    // its content and never lists sheets, selected or not.
    for (name, reason) in [
        ("unselected-formula.xlsx", "formula_refused"),
        ("macro.xlsx", "macro_or_binary_refused"),
        ("deep.xml.xlsx", "xml_depth_limit"),
    ] {
        let job = run(&frame(
            &config(json!({"profile":"demo","format":"xlsx"})),
            &fixture(name),
        ));
        assert_refused(&job, reason);
        assert!(job.document().get("available_sheets").is_none());
    }
}

#[test]
fn job_binary_refuses_every_hostile_fixture_and_accepts_valid_ones() {
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
        ("duplicate-cell.xlsx", "cell_duplicate"),
        ("duplicate-values.xlsx", "cell_value_duplicate"),
        ("deep.xml.xlsx", "xml_depth_limit"),
    ] {
        let job = run(&frame(
            &config(json!({"profile":"demo","format":"xlsx","sheet":"Inventory"})),
            &fixture(name),
        ));
        assert_refused(&job, reason);
    }
    let job = run(&frame(
        &config(json!({"profile":"demo","format":"xlsx","sheet":"missing"})),
        &fixture("valid.xlsx"),
    ));
    assert_refused(&job, "sheet_not_found");
    let job = run(&frame(
        &config(json!({"profile":"demo","format":"xlsx","sheet":"Inventory"})),
        &fixture("zero-exponents.xlsx"),
    ));
    assert_eq!(job.code, 0, "stderr={}", job.stderr);
    assert_eq!(job.document()["result"]["cells"][4]["value"]["value"], "0");
}

#[test]
fn job_mode_caps_input_at_the_demo_profile_before_the_guest() {
    let c = config(json!({"profile":"demo","format":"csv"}));
    let job = run(&frame(&c, &vec![b'a'; 10 * 1024 * 1024 + 1]));
    assert_refused(&job, "input_limit");
    assert!(job.report()["input_sha256"].is_null());
    let c = config(json!({"profile":"demo","format":"xlsx","sheet":"Inventory"}));
    let job = run(&frame(&c, &vec![0; 5 * 1024 * 1024 + 1]));
    assert_refused(&job, "input_limit");
}

#[test]
fn job_mode_refuses_bad_framing_and_configuration_as_failure() {
    let input = fixture("literal.csv");
    let mut short = 64u32.to_be_bytes().to_vec();
    short.extend_from_slice(b"{}");
    for (stdin, reason) in [
        (vec![0, 0, 1], "frame_truncated"),
        (frame(&[b' '; 4097], &input), "config_limit"),
        (short, "frame_truncated"),
        (frame(b"not json", &input), "config_invalid"),
        (
            frame(
                &config(json!({"profile":"platform","format":"csv"})),
                &input,
            ),
            "profile_not_demo",
        ),
        (
            frame(
                &config(json!({"profile":"demo","format":"csv","limits":{"rows":1}})),
                &input,
            ),
            "config_invalid",
        ),
        (
            frame(&config(json!({"profile":"demo","format":"ods"})), &input),
            "format_unsupported",
        ),
    ] {
        let job = run(&stdin);
        assert_eq!(job.code, 1, "{reason}: stderr={}", job.stderr);
        assert!(
            job.stdout.is_empty(),
            "{reason}: failure writes no document"
        );
        assert_eq!(job.report()["status"], "failure");
        assert_eq!(job.report()["reason"], reason);
    }
}

#[test]
fn job_mode_refuses_guest_bytes_that_differ_from_the_expected_digest() {
    let wasm = wasm_path();
    let wasm = wasm.to_str().unwrap();
    let wrong = "0".repeat(64);
    let c = config(json!({"profile":"demo","format":"csv"}));
    let job = run_with(
        &["--job", "--wasm", wasm, "--wasm-sha256", &wrong],
        &frame(&c, b"a\n"),
    );
    assert_eq!(job.code, 1);
    assert!(job.stdout.is_empty());
    assert_eq!(job.report()["reason"], "wasm_digest_mismatch");
    for args in [
        vec!["--job"],
        vec!["--job", "--wasm", wasm],
        vec!["--job", "--wasm", wasm, "--wasm-sha256", "SHA"],
        vec!["--job", "--wasm-sha256", &wrong, "--wasm", wasm, "extra"],
    ] {
        let job = run_with(&args, &frame(&c, b"a\n"));
        assert_eq!(job.code, 1, "{args:?}");
        assert!(job.stdout.is_empty());
        assert_eq!(job.report()["reason"], "usage", "{args:?}");
    }
}

#[test]
fn job_process_runs_exactly_one_job() {
    // Everything after the config is input to EOF: a second frame is never a
    // second job in the same process (one Engine per job).
    let c = config(json!({"profile":"demo","format":"csv"}));
    let mut stdin = frame(&c, b"a\n");
    stdin.extend_from_slice(&frame(&c, b"b\n"));
    let job = run(&stdin);
    assert_eq!(job.code, 0, "stderr={}", job.stderr);
    assert_eq!(
        job.report()["input_sha256"],
        format!("sha256:{}", hex(&stdin[4 + c.len()..]))
    );
}
