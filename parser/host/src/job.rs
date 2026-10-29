//! `parser-host --job --wasm <path> --wasm-sha256 <hex>`: exactly one parse
//! per process, so every job has its own `Engine`, epoch deadline and memory
//! (ADR-009 D9.1). The caller spawns it with an empty environment and only
//! its pipes.
//!
//! stdin: a 4-byte big-endian config length, the config JSON
//! (`{"profile":"demo","format":"csv"|"xlsx","sheet"?}`, at most 4,096
//! bytes), then the input bytes to EOF. Everything after the config is input:
//! there is no second job in a process.
//!
//! stdout: the guest's one JSON document, parsed or refused. stderr: one JSON
//! line, the `RunReport` without its result (the document travels on stdout
//! only) plus `status` and `reason`. Exit 0 parsed, 2 refused (no revision),
//! 1 failure (no stdout).
//!
//! The guest bytes are read from `--wasm` and must hash to `--wasm-sha256`,
//! the digest the caller verified against the image's parser-manifest.json.
//! The demo profile is fixed and no limit can be passed: the guest's
//! lower-only `limits` test channel is not reachable from a job.
use crate::{sha, Limits, Sandbox};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::Instant,
};

pub const CONFIG_LIMIT: usize = 4096;
pub const EXIT_PARSED: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const EXIT_REFUSED: i32 = 2;
const MIB: usize = 1024 * 1024;
const WASM_LIMIT: usize = 16 * MIB;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobConfig {
    profile: String,
    format: String,
    #[serde(default)]
    sheet: Option<String>,
}

struct Failure(&'static str);

struct Outcome {
    code: i32,
    document: Value,
    report: Value,
}

struct Args {
    wasm: String,
    wasm_sha256: String,
}

fn parse_args(args: &[String]) -> Result<Args, Failure> {
    let usage = Failure("usage");
    let [job, a, b, c, d] = args else {
        return Err(usage);
    };
    if job != "--job" {
        return Err(usage);
    }
    let (wasm, digest) = match (a.as_str(), c.as_str()) {
        ("--wasm", "--wasm-sha256") => (b, d),
        ("--wasm-sha256", "--wasm") => (d, b),
        _ => return Err(usage),
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x))
    {
        return Err(usage);
    }
    Ok(Args {
        wasm: wasm.clone(),
        wasm_sha256: digest.clone(),
    })
}

fn read_wasm(path: &str, expected: &str) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(WASM_LIMIT as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| Failure("wasm_unreadable"))?;
    if bytes.len() > WASM_LIMIT {
        return Err(Failure("wasm_unreadable"));
    }
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err(Failure("wasm_digest_mismatch"));
    }
    Ok(bytes)
}

fn read_config(stdin: &mut dyn Read) -> Result<Value, Failure> {
    let truncated = |_| Failure("frame_truncated");
    let mut prefix = [0u8; 4];
    stdin.read_exact(&mut prefix).map_err(truncated)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length > CONFIG_LIMIT {
        return Err(Failure("config_limit"));
    }
    let mut bytes = vec![0u8; length];
    stdin.read_exact(&mut bytes).map_err(truncated)?;
    let config: JobConfig =
        serde_json::from_slice(&bytes).map_err(|_| Failure("config_invalid"))?;
    if config.profile != "demo" {
        return Err(Failure("profile_not_demo"));
    }
    if config.format != "csv" && config.format != "xlsx" {
        return Err(Failure("format_unsupported"));
    }
    let mut guest = json!({"profile": "demo", "format": config.format});
    if let Some(sheet) = config.sheet {
        guest["sheet"] = json!(sheet);
    }
    Ok(guest)
}

fn input_cap(config: &Value) -> usize {
    if config["format"] == "csv" {
        10 * MIB
    } else {
        5 * MIB
    }
}

fn run(args: &[String], stdin: &mut dyn Read) -> Result<Outcome, Failure> {
    let args = parse_args(args)?;
    let wasm = read_wasm(&args.wasm, &args.wasm_sha256)?;
    let config = read_config(stdin)?;
    let cap = input_cap(&config);
    let mut input = Vec::new();
    stdin
        .take(cap as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|_| Failure("input_unreadable"))?;
    let wasm_sha256 = format!("sha256:{}", args.wasm_sha256);
    if input.len() > cap {
        // Refused before the guest exists; the rest of stdin is never read.
        return Ok(Outcome {
            code: EXIT_REFUSED,
            document: json!({"status":"refused","reason":"input_limit","no_revision_created":true}),
            report: json!({"status":"refused","reason":"input_limit","wasm_sha256":wasm_sha256,"input_sha256":null,"input_bytes_limit":cap}),
        });
    }
    let sandbox = Sandbox::new(&wasm).map_err(|_| Failure("wasm_invalid"))?;
    let report = sandbox
        .run(&input, &config, Limits::default())
        .map_err(|_| Failure("host_failure"))?;
    if report.wasm_sha256 != wasm_sha256 || report.input_sha256 != sha(&input) {
        return Err(Failure("host_failure"));
    }
    let code = match report.result["status"].as_str() {
        Some("parsed") => EXIT_PARSED,
        Some("refused") => EXIT_REFUSED,
        _ => return Err(Failure("guest_status_invalid")),
    };
    let mut line = serde_json::to_value(&report).map_err(|_| Failure("host_failure"))?;
    let document = line
        .as_object_mut()
        .and_then(|o| o.remove("result"))
        .ok_or(Failure("host_failure"))?;
    line["status"] = document["status"].clone();
    line["reason"] = document.get("reason").cloned().unwrap_or(Value::Null);
    Ok(Outcome {
        code,
        document,
        report: line,
    })
}

/// Run one job and return the process exit code.
pub fn main(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let started = Instant::now();
    let (code, document, mut report) = match run(args, stdin) {
        Ok(outcome) => (outcome.code, Some(outcome.document), outcome.report),
        Err(Failure(reason)) => (
            EXIT_FAILURE,
            None,
            json!({"status":"failure","reason":reason}),
        ),
    };
    // Whole-process time for this job, including module compilation.
    report["process_ms"] = json!(started.elapsed().as_millis() as u64);
    let mut code = code;
    if let Some(document) = document {
        let written =
            serde_json::to_writer(&mut *stdout, &document).is_ok() && stdout.flush().is_ok();
        if !written {
            code = EXIT_FAILURE;
            report = json!({"status":"failure","reason":"stdout_write"});
        }
    }
    let _ = serde_json::to_writer(&mut *stderr, &report);
    let _ = stderr.write_all(b"\n");
    let _ = stderr.flush();
    code
}
