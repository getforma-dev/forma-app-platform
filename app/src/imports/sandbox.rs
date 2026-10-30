//! One `parser-host --job` process per parse (ADR-009 D9.1, D9.2).
//!
//! The image's `parser-manifest.json` names the guest and host bytes the
//! image build produced; `verify_manifest` checks them at startup and refuses
//! a mismatch. That is an integrity check, not the release allowlist: a
//! release separately refuses parser bytes (guest, host and sources) without
//! a passing, CI-verified image-bytes qualification record
//! (`scripts/check-parser-record.py`).
//!
//! Each parse spawns `parser-host --job` under `supervise::run_confined` in the
//! job slot (2.5 GiB RSS line, the guest's 120 s deadline plus a 5 s
//! backstop). The host re-hashes the guest against the verified digest before
//! it compiles it, and its report must name that digest. Its report must also
//! name the caller's expected input digest, after exactly the expected length
//! was fed, or the parse is a failure.
use crate::supervise::{run_confined, Confinement, Killed};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

/// Set by the image (app/Dockerfile) to the manifest its build wrote.
pub const MANIFEST_ENV: &str = "GENUI_PARSER_MANIFEST_FILE";
pub const MANIFEST_SCHEMA: &str = "genui-parser-manifest/1";
/// The job slot's RSS line: `parser-host` (2 GiB guest plus host) or the
/// normalize child, never both (ADR-009 D9.2).
pub const SLOT_RSS_LIMIT_BYTES: u64 = 2560 * 1024 * 1024;
pub const GUEST_DEADLINE: Duration = Duration::from_secs(120);
pub const BACKSTOP: Duration = Duration::from_secs(5);
pub const CONFIG_LIMIT_BYTES: usize = 4096;
const MANIFEST_LIMIT_BYTES: u64 = 64 * 1024;
const WASM_LIMIT_BYTES: u64 = 16 * 1024 * 1024;
const HOST_LIMIT_BYTES: u64 = 256 * 1024 * 1024;

static IDENTITY: OnceLock<ParserIdentity> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Xlsx,
}

#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub format: Format,
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserIdentity {
    pub host: PathBuf,
    pub host_sha256: String,
    pub wasm: PathBuf,
    pub wasm_sha256: String,
    pub manifest_sha256: String,
}

/// What the caller knows the whole input to be (the upload's `content_digest`
/// and `byte_length`). A parse completes only for exactly these bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedInput {
    /// 64 lowercase hex characters, without the `sha256:` prefix.
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct ParseOutput {
    pub document: serde_json::Value,
    pub report: serde_json::Value,
    pub peak_rss_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseRefusal {
    Refused {
        reason: String,
        available_sheets: Option<Vec<String>>,
    },
    Deadline,
    ResourceLimit,
    Crashed,
    Failure(&'static str),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    guest: Artifact,
    host: Artifact,
    // Source binding for qualification; recorded, not interpreted here.
    #[allow(dead_code)]
    source: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: PathBuf,
    sha256: String,
    bytes: u64,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>, &'static str> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "parser artifact unreadable")?;
    if !metadata.file_type().is_file() {
        return Err("parser artifact not a regular file");
    }
    let mut bytes = Vec::new();
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|_| "parser artifact unreadable")?;
    if bytes.len() as u64 > limit {
        return Err("parser artifact too large");
    }
    Ok(bytes)
}

fn check(artifact: &Artifact, limit: u64, executable: bool) -> Result<(), &'static str> {
    if !artifact.path.is_absolute() {
        return Err("parser artifact path not absolute");
    }
    if artifact.sha256.len() != 64
        || !artifact
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("parser artifact digest invalid");
    }
    let bytes = read_regular(&artifact.path, limit)?;
    if bytes.len() as u64 != artifact.bytes {
        return Err("parser artifact size mismatch");
    }
    if hex(&bytes) != artifact.sha256 {
        return Err("parser artifact digest mismatch");
    }
    if executable {
        let mode = std::fs::metadata(&artifact.path)
            .map_err(|_| "parser artifact unreadable")?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err("parser host not executable");
        }
    }
    Ok(())
}

/// Verify the manifest named by `GENUI_PARSER_MANIFEST_FILE` and install its
/// identity for `parse`. Startup refuses on any mismatch.
pub fn verify_manifest() -> Result<ParserIdentity, &'static str> {
    let path = std::env::var_os(MANIFEST_ENV).ok_or("parser manifest not configured")?;
    let identity = verify_manifest_at(Path::new(&path))?;
    let installed = IDENTITY.get_or_init(|| identity.clone());
    if *installed != identity {
        return Err("parser manifest changed after startup");
    }
    Ok(identity)
}

/// Check that the guest and host on disk are exactly the bytes the manifest
/// names (integrity only; the release record is the qualification gate).
pub fn verify_manifest_at(path: &Path) -> Result<ParserIdentity, &'static str> {
    let raw = read_regular(path, MANIFEST_LIMIT_BYTES).map_err(|_| "parser manifest unreadable")?;
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(|_| "parser manifest invalid")?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err("parser manifest schema unknown");
    }
    check(&manifest.guest, WASM_LIMIT_BYTES, false)?;
    check(&manifest.host, HOST_LIMIT_BYTES, true)?;
    Ok(ParserIdentity {
        host: manifest.host.path,
        host_sha256: manifest.host.sha256,
        wasm: manifest.guest.path,
        wasm_sha256: manifest.guest.sha256,
        manifest_sha256: hex(&raw),
    })
}

/// Parse with the identity `verify_manifest` installed at startup.
pub async fn parse(
    input: impl AsyncRead + Unpin + Send + 'static,
    expected: &ExpectedInput,
    config: &ParseConfig,
    deadline: Duration,
) -> Result<ParseOutput, ParseRefusal> {
    let identity = IDENTITY
        .get()
        .ok_or(ParseRefusal::Failure("parser_identity_missing"))?;
    parse_with(identity, input, expected, config, deadline).await
}

fn frame_config(config: &ParseConfig) -> Vec<u8> {
    let format = match config.format {
        Format::Csv => "csv",
        Format::Xlsx => "xlsx",
    };
    let mut value = json!({"profile": "demo", "format": format});
    if let Some(sheet) = &config.sheet {
        value["sheet"] = json!(sheet);
    }
    serde_json::to_vec(&value).expect("a JSON object serializes")
}

/// Counts the bytes the caller's input yields on their way to the child.
struct Counted<R> {
    inner: R,
    bytes: Arc<AtomicU64>,
}

impl<R: AsyncRead + Unpin> AsyncRead for Counted<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = polled {
            let read = (buf.filled().len() - before) as u64;
            self.bytes.fetch_add(read, Ordering::Relaxed);
        }
        polled
    }
}

/// Run one `parser-host --job` process for one parse. The result stands only
/// for `expected`: the host's report must name its digest and the child must
/// have been fed exactly its length, so a source that ends early without an
/// error never completes as the whole input.
pub async fn parse_with(
    identity: &ParserIdentity,
    input: impl AsyncRead + Unpin + Send + 'static,
    expected: &ExpectedInput,
    config: &ParseConfig,
    deadline: Duration,
) -> Result<ParseOutput, ParseRefusal> {
    let config = frame_config(config);
    if config.len() > CONFIG_LIMIT_BYTES {
        return Err(ParseRefusal::Refused {
            reason: "config_limit".into(),
            available_sheets: None,
        });
    }
    let mut header = (config.len() as u32).to_be_bytes().to_vec();
    header.extend_from_slice(&config);
    let mut command = tokio::process::Command::new(&identity.host);
    command
        .arg("--job")
        .arg("--wasm")
        .arg(&identity.wasm)
        .arg("--wasm-sha256")
        .arg(&identity.wasm_sha256);
    let confinement = Confinement {
        rss_limit_bytes: SLOT_RSS_LIMIT_BYTES,
        deadline: deadline.min(GUEST_DEADLINE) + BACKSTOP,
        nofile: 64,
        core: 0,
    };
    let fed = Arc::new(AtomicU64::new(0));
    let input = Counted {
        inner: input,
        bytes: Arc::clone(&fed),
    };
    let output = run_confined(
        command,
        AsyncReadExt::chain(std::io::Cursor::new(header), input),
        &confinement,
    )
    .await
    .map_err(|killed| match killed {
        Killed::Deadline => ParseRefusal::Deadline,
        Killed::ResourceLimit => ParseRefusal::ResourceLimit,
        Killed::Crashed => ParseRefusal::Crashed,
        Killed::InputFailed => ParseRefusal::Failure("input_read"),
    })?;
    let code = output.status.code();
    if code != Some(0) && code != Some(2) {
        return Err(ParseRefusal::Failure("parser_failure"));
    }
    let text = std::str::from_utf8(&output.stderr)
        .map_err(|_| ParseRefusal::Failure("parser_report_invalid"))?;
    let mut lines = text.lines();
    let report: Value = match (lines.next(), lines.next()) {
        (Some(line), None) => serde_json::from_str(line)
            .map_err(|_| ParseRefusal::Failure("parser_report_invalid"))?,
        _ => return Err(ParseRefusal::Failure("parser_report_invalid")),
    };
    if report["wasm_sha256"] != format!("sha256:{}", identity.wasm_sha256) {
        return Err(ParseRefusal::Failure("parser_identity_mismatch"));
    }
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| ParseRefusal::Failure("parser_output_invalid"))?;
    // The child read to EOF before it reported a digest, so `fed` is final.
    let whole_input = report["input_sha256"] == format!("sha256:{}", expected.sha256)
        && fed.load(Ordering::Relaxed) == expected.bytes;
    match (code, document["status"].as_str()) {
        (Some(0), Some("parsed")) if !whole_input => {
            Err(ParseRefusal::Failure("input_digest_mismatch"))
        }
        (Some(0), Some("parsed")) => Ok(ParseOutput {
            document,
            report,
            peak_rss_bytes: output.peak_rss_bytes,
        }),
        (Some(2), Some("refused")) => {
            let reason = document["reason"]
                .as_str()
                .ok_or(ParseRefusal::Failure("parser_output_invalid"))?
                .to_owned();
            // Only `input_limit` is decided before the whole input is read,
            // and its report names no digest; every other refusal is about
            // exactly the expected bytes.
            let before_reading = reason == "input_limit" && report["input_sha256"].is_null();
            if !before_reading && !whole_input {
                return Err(ParseRefusal::Failure("input_digest_mismatch"));
            }
            let available_sheets = match document.get("available_sheets") {
                None => None,
                Some(sheets) => Some(
                    serde_json::from_value(sheets.clone())
                        .map_err(|_| ParseRefusal::Failure("parser_output_invalid"))?,
                ),
            };
            Err(ParseRefusal::Refused {
                reason,
                available_sheets,
            })
        }
        _ => Err(ParseRefusal::Failure("parser_status_mismatch")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{os::unix::fs::PermissionsExt, path::Path};

    fn hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    struct Fixture {
        _directory: TempDir,
        identity: ParserIdentity,
    }
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            // Parallel tests can read the same clock value (microsecond
            // resolution on macOS); the counter keeps names distinct.
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "genui-parser-{tag}-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    /// A stand-in `parser-host`; `$INPUT_HEX` is the digest it reports for
    /// `seen`, the input an honest host would have read.
    fn stub_seeing(body: &str, seen: &[u8]) -> Fixture {
        let directory = TempDir::new("stub");
        let wasm = directory.0.join("parser_guest.wasm");
        std::fs::write(&wasm, b"\0asm\x01\0\0\0stub").unwrap();
        let wasm_sha256 = hex(&std::fs::read(&wasm).unwrap());
        let host = directory.0.join("parser-host");
        let script = format!(
            "#!/bin/sh\nWASM_HEX={wasm_sha256}\nINPUT_HEX={}\n{body}\n",
            hex(seen)
        );
        std::fs::write(&host, &script).unwrap();
        std::fs::set_permissions(&host, std::fs::Permissions::from_mode(0o755)).unwrap();
        let identity = ParserIdentity {
            host_sha256: hex(script.as_bytes()),
            host,
            wasm,
            wasm_sha256,
            manifest_sha256: "0".repeat(64),
        };
        Fixture {
            _directory: directory,
            identity,
        }
    }
    fn stub(body: &str) -> Fixture {
        stub_seeing(body, b"a\n")
    }
    fn expect(input: &[u8]) -> ExpectedInput {
        ExpectedInput {
            sha256: hex(input),
            bytes: input.len() as u64,
        }
    }
    const REPORTING_STUB: &str = r#"bytes=$(/usr/bin/wc -c | /usr/bin/tr -d ' ')
envs=$(/usr/bin/env | /usr/bin/grep -v -E '^(PWD|SHLVL|_|OLDPWD)=' | /usr/bin/tr '\n' ';')
printf '{"status":"parsed","result":{"pid":%s,"stdin_bytes":%s,"env":"%s","args":"%s"}}' "$$" "$bytes" "$envs" "$*"
printf '{"status":"parsed","reason":null,"wasm_sha256":"sha256:%s","input_sha256":"sha256:%s"}\n' "$WASM_HEX" "$INPUT_HEX" >&2
exit 0"#;
    fn csv() -> ParseConfig {
        ParseConfig {
            format: Format::Csv,
            sheet: None,
        }
    }

    #[tokio::test]
    async fn parser_host_runs_one_job_per_process_with_cleared_environment() {
        std::env::set_var("GENUI_PARSER_LEAK_PROBE", "must-not-leak");
        let input = b"a,b\n1,2\n".to_vec();
        let fixture = stub_seeing(REPORTING_STUB, &input);
        let mut pids = Vec::new();
        for _ in 0..2 {
            let output = parse_with(
                &fixture.identity,
                std::io::Cursor::new(input.clone()),
                &expect(&input),
                &csv(),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            let result = &output.document["result"];
            // Nothing from this process's environment reaches the parser.
            assert_eq!(result["env"], "", "{result}");
            // Exactly the framed job: 4-byte length, config, input to EOF.
            let config = br#"{"format":"csv","profile":"demo"}"#;
            assert_eq!(result["stdin_bytes"], 4 + config.len() + input.len());
            assert_eq!(
                result["args"],
                format!(
                    "--job --wasm {} --wasm-sha256 {}",
                    fixture.identity.wasm.display(),
                    fixture.identity.wasm_sha256
                )
            );
            pids.push(result["pid"].as_u64().unwrap());
        }
        assert_ne!(pids[0], pids[1], "each parse is its own process");
    }

    #[tokio::test]
    async fn frame_carries_the_selected_sheet_and_demo_profile_only() {
        let fixture = stub_seeing(
            r#"config=$(/usr/bin/tail -c +5)
printf '{"status":"parsed","result":{"config":%s}}' "$config"
printf '{"status":"parsed","wasm_sha256":"sha256:%s","input_sha256":"sha256:%s"}\n' "$WASM_HEX" "$INPUT_HEX" >&2"#,
            b"",
        );
        let output = parse_with(
            &fixture.identity,
            // No input bytes: everything after the length prefix is the config.
            std::io::Cursor::new(Vec::new()),
            &expect(b""),
            &ParseConfig {
                format: Format::Xlsx,
                sheet: Some("Inventory".into()),
            },
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(
            output.document["result"]["config"],
            json!({"format":"xlsx","profile":"demo","sheet":"Inventory"})
        );
    }

    #[tokio::test]
    async fn refusal_exit_two_returns_reason_and_available_sheets() {
        let fixture = stub_seeing(
            r#"/bin/cat > /dev/null
printf '{"status":"refused","reason":"sheet_selection_required","no_revision_created":true,"available_sheets":["Inventory","Other"]}'
printf '{"status":"refused","reason":"sheet_selection_required","wasm_sha256":"sha256:%s","input_sha256":"sha256:%s"}\n' "$WASM_HEX" "$INPUT_HEX" >&2
exit 2"#,
            b"PK",
        );
        let refusal = parse_with(
            &fixture.identity,
            std::io::Cursor::new(b"PK".to_vec()),
            &expect(b"PK"),
            &ParseConfig {
                format: Format::Xlsx,
                sheet: None,
            },
            Duration::from_secs(10),
        )
        .await
        .unwrap_err();
        assert_eq!(
            refusal,
            ParseRefusal::Refused {
                reason: "sheet_selection_required".into(),
                available_sheets: Some(vec!["Inventory".into(), "Other".into()]),
            }
        );
    }

    #[tokio::test]
    async fn protocol_and_identity_violations_are_failures() {
        for (body, expected) in [
            (
                "/bin/cat > /dev/null\necho '{\"reason\":\"config_invalid\",\"status\":\"failure\"}' >&2\nexit 1",
                ParseRefusal::Failure("parser_failure"),
            ),
            (
                // A report naming other guest bytes is never accepted.
                "/bin/cat > /dev/null\nprintf '{\"status\":\"parsed\"}'\nprintf '{\"status\":\"parsed\",\"wasm_sha256\":\"sha256:%s\"}\\n' 0000 >&2",
                ParseRefusal::Failure("parser_identity_mismatch"),
            ),
            (
                "/bin/cat > /dev/null\nprintf 'not json'\nprintf '{\"status\":\"parsed\",\"wasm_sha256\":\"sha256:%s\"}\\n' \"$WASM_HEX\" >&2",
                ParseRefusal::Failure("parser_output_invalid"),
            ),
            (
                // Exit status and document disagree.
                "/bin/cat > /dev/null\nprintf '{\"status\":\"refused\",\"reason\":\"x\"}'\nprintf '{\"status\":\"refused\",\"wasm_sha256\":\"sha256:%s\"}\\n' \"$WASM_HEX\" >&2",
                ParseRefusal::Failure("parser_status_mismatch"),
            ),
            (
                "/bin/cat > /dev/null\nexit 7",
                ParseRefusal::Failure("parser_failure"),
            ),
            (
                "/bin/cat > /dev/null\nkill -KILL $$",
                ParseRefusal::Crashed,
            ),
        ] {
            let fixture = stub(body);
            let outcome = parse_with(
                &fixture.identity,
                std::io::Cursor::new(b"a\n".to_vec()),
                &expect(b"a\n"),
                &csv(),
                Duration::from_secs(10),
            )
            .await;
            assert_eq!(outcome.unwrap_err(), expected, "{body}");
        }
    }

    #[tokio::test]
    async fn input_that_ends_early_without_an_error_is_never_parsed() {
        // The source stops at 8 of 12 bytes with Ok(0): to the child that is
        // a complete input. The parse is bound to the expected digest and
        // length, so it never completes as the whole upload (review m4).
        let whole = b"a,b\n1,2\n3,4\n";
        let early = &whole[..8];
        let honest = stub_seeing(REPORTING_STUB, early);
        // A host that reported the whole digest anyway is caught by length.
        let lying = stub_seeing(REPORTING_STUB, whole);
        for fixture in [&honest, &lying] {
            let outcome = parse_with(
                &fixture.identity,
                std::io::Cursor::new(early.to_vec()),
                &expect(whole),
                &csv(),
                Duration::from_secs(10),
            )
            .await;
            assert_eq!(
                outcome.unwrap_err(),
                ParseRefusal::Failure("input_digest_mismatch")
            );
        }
        // A refusal is bound the same way; only `input_limit`, which the host
        // decides before it has read (and hashed) the whole input, has none.
        for (reason, digest, expected) in [
            (
                "formula_refused",
                "\"sha256:'\"$INPUT_HEX\"'\"",
                ParseRefusal::Failure("input_digest_mismatch"),
            ),
            (
                "formula_refused",
                "null",
                ParseRefusal::Failure("input_digest_mismatch"),
            ),
            (
                "input_limit",
                "null",
                ParseRefusal::Refused {
                    reason: "input_limit".into(),
                    available_sheets: None,
                },
            ),
        ] {
            let body = format!(
                "/bin/cat > /dev/null\nprintf '{{\"status\":\"refused\",\"reason\":\"{reason}\",\"no_revision_created\":true}}'\nprintf '{{\"status\":\"refused\",\"reason\":\"{reason}\",\"wasm_sha256\":\"sha256:%s\",\"input_sha256\":{digest}}}\\n' \"$WASM_HEX\" >&2\nexit 2"
            );
            let fixture = stub_seeing(&body, early);
            let outcome = parse_with(
                &fixture.identity,
                std::io::Cursor::new(early.to_vec()),
                &expect(whole),
                &csv(),
                Duration::from_secs(10),
            )
            .await;
            assert_eq!(outcome.unwrap_err(), expected, "{reason} {digest}");
        }
    }

    #[tokio::test]
    async fn parse_without_a_verified_manifest_is_refused() {
        // Unit tests never install the process-wide identity.
        assert_eq!(
            parse(
                std::io::Cursor::new(b"a\n".to_vec()),
                &expect(b"a\n"),
                &csv(),
                Duration::from_secs(1)
            )
            .await
            .unwrap_err(),
            ParseRefusal::Failure("parser_identity_missing")
        );
    }

    fn manifest(directory: &Path, wasm: &[u8], host: &[u8]) -> PathBuf {
        let wasm_path = directory.join("parser_guest.wasm");
        let host_path = directory.join("parser-host");
        std::fs::write(&wasm_path, wasm).unwrap();
        std::fs::write(&host_path, host).unwrap();
        std::fs::set_permissions(&host_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = directory.join("parser-manifest.json");
        let document = json!({
            "schema": "genui-parser-manifest/1",
            "guest": {"path": wasm_path, "sha256": hex(wasm), "bytes": wasm.len()},
            "host": {"path": host_path, "sha256": hex(host), "bytes": host.len()},
            "source": {"sources_sha256": "c".repeat(64), "cargo_lock_sha256": "d".repeat(64)}
        });
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        path
    }
    fn edit(path: &Path, change: impl FnOnce(&mut Value)) {
        let mut document: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        change(&mut document);
        std::fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
    }

    #[test]
    fn manifest_matching_image_bytes_yields_identity() {
        let directory = TempDir::new("manifest");
        let path = manifest(&directory.0, b"\0asm guest", b"#!/bin/sh\n");
        let identity = verify_manifest_at(&path).unwrap();
        assert_eq!(identity.wasm_sha256, hex(b"\0asm guest"));
        assert_eq!(identity.host_sha256, hex(b"#!/bin/sh\n"));
        assert_eq!(identity.wasm, directory.0.join("parser_guest.wasm"));
        assert_eq!(
            identity.manifest_sha256,
            hex(&std::fs::read(&path).unwrap())
        );
    }

    #[test]
    fn manifest_mismatch_is_refused_at_startup() {
        type Tamper = Box<dyn Fn(&Path, &Path)>;
        let cases: Vec<(&str, Tamper)> = vec![
            (
                "guest bytes changed",
                Box::new(|d, _| {
                    std::fs::write(d.join("parser_guest.wasm"), b"\0asm other!").unwrap()
                }),
            ),
            (
                "host bytes changed",
                Box::new(|d, _| std::fs::write(d.join("parser-host"), b"#!/bin/sh\n#").unwrap()),
            ),
            (
                "guest missing",
                Box::new(|d, _| std::fs::remove_file(d.join("parser_guest.wasm")).unwrap()),
            ),
            (
                "guest is a symlink",
                Box::new(|d, _| {
                    let real = d.join("real.wasm");
                    std::fs::rename(d.join("parser_guest.wasm"), &real).unwrap();
                    std::os::unix::fs::symlink(&real, d.join("parser_guest.wasm")).unwrap();
                }),
            ),
            (
                "host not executable",
                Box::new(|d, _| {
                    std::fs::set_permissions(
                        d.join("parser-host"),
                        std::fs::Permissions::from_mode(0o644),
                    )
                    .unwrap()
                }),
            ),
            (
                "relative path",
                Box::new(|_, m| edit(m, |v| v["guest"]["path"] = json!("parser_guest.wasm"))),
            ),
            (
                "size differs",
                Box::new(|_, m| edit(m, |v| v["guest"]["bytes"] = json!(1))),
            ),
            (
                "digest not lowercase hex",
                Box::new(|_, m| {
                    edit(m, |v| {
                        let upper = v["guest"]["sha256"].as_str().unwrap().to_uppercase();
                        v["guest"]["sha256"] = json!(upper)
                    })
                }),
            ),
            (
                "unknown schema",
                Box::new(|_, m| edit(m, |v| v["schema"] = json!("genui-parser-manifest/2"))),
            ),
            (
                "unknown field",
                Box::new(|_, m| edit(m, |v| v["allowlist"] = json!([]))),
            ),
            (
                "not json",
                Box::new(|_, m| std::fs::write(m, b"{").unwrap()),
            ),
        ];
        for (name, change) in cases {
            let directory = TempDir::new("tamper");
            let path = manifest(&directory.0, b"\0asm guest", b"#!/bin/sh\n");
            change(&directory.0, &path);
            assert!(verify_manifest_at(&path).is_err(), "{name}");
        }
        assert!(verify_manifest_at(Path::new("/nonexistent/parser-manifest.json")).is_err());
    }

    /// Against the actual parser: `GENUI_TEST_PARSER_MANIFEST` names a
    /// parser-manifest.json whose host and guest run on this machine.
    #[tokio::test]
    #[ignore = "needs a built parser-host and guest named by GENUI_TEST_PARSER_MANIFEST"]
    async fn actual_parser_host_parses_refuses_and_lists_sheets() {
        let manifest = std::env::var_os("GENUI_TEST_PARSER_MANIFEST").expect("manifest path");
        let identity = verify_manifest_at(Path::new(&manifest)).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../parser/fixtures");
        let csv_bytes = std::fs::read(fixtures.join("literal.csv")).unwrap();
        let output = parse_with(
            &identity,
            std::io::Cursor::new(csv_bytes.clone()),
            &expect(&csv_bytes),
            &csv(),
            Duration::from_secs(120),
        )
        .await
        .unwrap();
        assert_eq!(output.document["status"], "parsed");
        assert_eq!(output.document["result"]["rows"], 3);
        assert_eq!(
            output.report["input_sha256"],
            format!("sha256:{}", hex(&csv_bytes))
        );
        assert!(output.peak_rss_bytes > 0);
        // The real host parses a source that ended early; the parent refuses it.
        let early = csv_bytes[..csv_bytes.len() - 3].to_vec();
        let refusal = parse_with(
            &identity,
            std::io::Cursor::new(early),
            &expect(&csv_bytes),
            &csv(),
            Duration::from_secs(120),
        )
        .await
        .unwrap_err();
        assert_eq!(refusal, ParseRefusal::Failure("input_digest_mismatch"));
        let workbook = std::fs::read(fixtures.join("valid.xlsx")).unwrap();
        let refusal = parse_with(
            &identity,
            std::io::Cursor::new(workbook.clone()),
            &expect(&workbook),
            &ParseConfig {
                format: Format::Xlsx,
                sheet: None,
            },
            Duration::from_secs(120),
        )
        .await
        .unwrap_err();
        assert_eq!(
            refusal,
            ParseRefusal::Refused {
                reason: "sheet_selection_required".into(),
                available_sheets: Some(vec!["Inventory".into(), "Other".into()]),
            }
        );
        let output = parse_with(
            &identity,
            std::io::Cursor::new(workbook.clone()),
            &expect(&workbook),
            &ParseConfig {
                format: Format::Xlsx,
                sheet: Some("Other".into()),
            },
            Duration::from_secs(120),
        )
        .await
        .unwrap();
        assert_eq!(
            output.document["result"]["cells"][0]["value"]["value"],
            "Other sheet only"
        );
        let formula = std::fs::read(fixtures.join("formula.xlsx")).unwrap();
        let refusal = parse_with(
            &identity,
            std::io::Cursor::new(formula.clone()),
            &expect(&formula),
            &ParseConfig {
                format: Format::Xlsx,
                sheet: Some("Inventory".into()),
            },
            Duration::from_secs(120),
        )
        .await
        .unwrap_err();
        assert_eq!(
            refusal,
            ParseRefusal::Refused {
                reason: "formula_refused".into(),
                available_sheets: None,
            }
        );
        println!(
            "ACTUAL_PARSER host_sha256={} wasm_sha256={} peak_rss_bytes={}",
            identity.host_sha256, identity.wasm_sha256, output.peak_rss_bytes
        );
    }
}
