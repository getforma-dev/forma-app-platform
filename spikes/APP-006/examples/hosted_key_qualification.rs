//! Operator-only, single-process qualification of the canonical verifier.
//! Bearer input is accepted once through a pipe, never an argument or environment
//! variable. This executable does not listen, exchange codes or create sessions.
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::FileTypeExt;
use std::sync::mpsc;
use std::time::{Duration, Instant as StdInstant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use genui_auth_verifier::{
    Config, ContractProfile, Diagnostics, Refusal, Verifier, MAX_ACCESS_KEYS, MAX_TOKEN_BYTES,
};
use ring::{digest, rand::SecureRandom};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::{timeout, Instant};

const INPUT_CAP: u64 = 16 * 1024;
const INPUT_DEADLINE: Duration = Duration::from_secs(2);
const RUN_DEADLINE: Duration = Duration::from_secs(90);
const REAL_COOLDOWN: Duration = Duration::from_secs(61);

fn arm_watchdog(deadline: StdInstant) {
    // Covers synchronous input/hash/output and runtime shutdown as well as the
    // async campaign. Exit124 without a complete report is always a failed run.
    // This fixed production bound has no argument/env/input override.
    if std::thread::Builder::new()
        .name("qualification-deadline".into())
        .spawn(move || {
            loop {
                let remaining = deadline.saturating_duration_since(StdInstant::now());
                if remaining.is_zero() {
                    break;
                }
                std::thread::sleep(remaining);
            }
            std::process::exit(124);
        })
        .is_err()
    {
        std::process::exit(125);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema_version: u8,
    synthetic_tenant: bool,
    run_id: String,
    declared_consumer_revision: String,
    declared_provider_revision: String,
    expected_tenant: String,
    expected_subject: String,
    issuer: String,
    audience: String,
    jwks_url: String,
    max_access_lifetime_seconds: u64,
    expires_at: u64,
    access_token: String,
}

fn unix_now() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| "clock_unavailable")
}

fn source_pin(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn provider_id(value: &str, prefix: &str) -> bool {
    let Some(id) = value.strip_prefix(prefix) else {
        return false;
    };
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

fn parse_input(bytes: &[u8], now: u64) -> Result<Input, &'static str> {
    if bytes.is_empty() || bytes.len() > INPUT_CAP as usize {
        return Err("input_size");
    }
    let input: Input = serde_json::from_slice(bytes).map_err(|_| "input_schema")?;
    if input.schema_version != 1
        || !input.synthetic_tenant
        || input.run_id.is_empty()
        || input.run_id.len() > 64
        || !input
            .run_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !source_pin(&input.declared_consumer_revision)
        || !source_pin(&input.declared_provider_revision)
        || !provider_id(&input.expected_tenant, "ten_")
        || !provider_id(&input.expected_subject, "usr_")
        || input.issuer != "gatewasm"
        || input.audience != "gatewasm-app"
        || input.jwks_url != "https://auth-staging.getforma.dev/auth/jwks"
        || input.max_access_lifetime_seconds != 900
        || !(90..=180).contains(&input.expires_at.saturating_sub(now))
        || input.access_token.is_empty()
        || input.access_token.len() > MAX_TOKEN_BYTES
    {
        return Err("input_contract");
    }
    Ok(input)
}

fn read_pipe() -> Result<Vec<u8>, &'static str> {
    let fd = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| "input_descriptor")?;
    let file = File::from(fd);
    if !file
        .metadata()
        .map_err(|_| "input_descriptor")?
        .file_type()
        .is_fifo()
    {
        return Err("input_requires_pipe");
    }
    let (tx, rx) = mpsc::sync_channel(1);
    // A bounded parent wait plus process::exit in main also closes the reader
    // if the writer withholds EOF. No Tokio blocking thread prevents shutdown.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::with_capacity(INPUT_CAP as usize + 1);
        let result = file
            .take(INPUT_CAP + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|_| "input_read");
        let _ = tx.send(result);
    });
    let received = rx
        .recv_timeout(INPUT_DEADLINE)
        .map_err(|_| "input_deadline")?;
    reader.join().map_err(|_| "input_reader")?;
    received
}

fn sha256(bytes: &[u8]) -> String {
    digest::digest(&digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn executable_digest() -> Result<String, &'static str> {
    let path = std::env::current_exe().map_err(|_| "executable_identity")?;
    let mut file = File::open(path).map_err(|_| "executable_identity")?;
    let meta = file.metadata().map_err(|_| "executable_identity")?;
    if !meta.is_file() || meta.len() > 256 * 1024 * 1024 {
        return Err("executable_identity");
    }
    let mut context = digest::Context::new(&digest::SHA256);
    let mut buffer = [0; 16 * 1024];
    let mut remaining = meta.len();
    while remaining > 0 {
        let cap = remaining.min(buffer.len() as u64) as usize;
        let read = file
            .read(&mut buffer[..cap])
            .map_err(|_| "executable_identity")?;
        if read == 0 {
            return Err("executable_identity");
        }
        context.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file.read(&mut [0; 1]).map_err(|_| "executable_identity")? != 0 {
        return Err("executable_identity");
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn control_token(token: &str, known_kid: &str) -> Result<String, &'static str> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("control_shape");
    }
    let mut random = [0; 8];
    ring::rand::SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| "control_random")?;
    let kid = URL_SAFE_NO_PAD.encode(random);
    if kid == known_kid {
        return Err("control_collision");
    }
    let header = json!({"alg":"EdDSA", "typ":"JWT", "kid":kid});
    Ok(format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        parts[1],
        parts[2]
    ))
}

fn counters(d: Diagnostics) -> Value {
    json!({"reserved_refresh_attempts": d.refresh_attempts,
        "accepted_keysets":d.accepted_keysets, "cached_access_keys":d.cached_access_keys,
        "cache_generation":d.generation})
}

fn expected_counts(d: Diagnostics, expected: u64) -> bool {
    d.refresh_attempts == expected
        && d.accepted_keysets == expected
        && d.generation == expected
        && (1..=MAX_ACCESS_KEYS).contains(&d.cached_access_keys)
}

async fn observe(verifier: &Verifier) -> Result<Diagnostics, &'static str> {
    verifier
        .diagnostics()
        .await
        .map_err(|_| "diagnostics_unavailable")
}

async fn campaign(input: &Input, checks: &mut Vec<Value>) -> Result<(), &'static str> {
    // This is the production constructor and real clock. Exactly one verifier
    // lives through the entire campaign; no private cache seam or retry is used.
    let verifier = Verifier::new(Config {
        profile: ContractProfile::Forma248AccessOnly,
        issuer: input.issuer.clone(),
        audience: input.audience.clone(),
        jwks_url: input.jwks_url.clone(),
        max_access_lifetime: Duration::from_secs(input.max_access_lifetime_seconds),
    })
    .map_err(|_| "verifier_configuration")?;
    let warm = verifier
        .verify(&input.access_token, &input.expected_tenant)
        .await
        .map_err(|_| "warm_identity_refused")?;
    let warm_finished = Instant::now();
    let initial = observe(&verifier).await?;
    if warm.subject != input.expected_subject
        || warm.expires_at.saturating_sub(unix_now()?) < 100
        || !expected_counts(initial, 1)
    {
        return Err("warm_identity_or_counters");
    }
    checks.push(
        json!({"case":"warm_verified_identity", "passed":true, "counters":counters(initial)}),
    );

    let control = control_token(&input.access_token, &warm.key_id)?;
    let first = verifier.verify(&control, &input.expected_tenant).await;
    let first_counts = observe(&verifier).await?;
    if first != Err(Refusal::RefreshThrottled) || first_counts != initial {
        return Err("immediate_unknown_not_throttled");
    }
    checks.push(json!({"case":"unknown_inside_cooldown", "passed":true,
        "refusal":"RefreshThrottled", "counters":counters(first_counts)}));

    tokio::time::sleep_until(warm_finished + REAL_COOLDOWN).await;
    let elapsed = warm_finished.elapsed();
    if elapsed < REAL_COOLDOWN || unix_now()? >= input.expires_at {
        return Err("real_interval_or_input_expired");
    }
    let second = verifier.verify(&control, &input.expected_tenant).await;
    let refreshed = observe(&verifier).await?;
    if second != Err(Refusal::UnknownKey) || !expected_counts(refreshed, 2) {
        return Err("unknown_refresh_not_observed");
    }
    checks.push(json!({"case":"unknown_after_real_cooldown", "passed":true,
        "refusal":"UnknownKey", "real_wait_milliseconds":elapsed.as_millis(),
        "counters":counters(refreshed)}));

    let third = verifier.verify(&control, &input.expected_tenant).await;
    let repeated = observe(&verifier).await?;
    if third != Err(Refusal::RefreshThrottled) || repeated != refreshed {
        return Err("repeat_unknown_not_throttled");
    }
    checks.push(
        json!({"case":"repeat_unknown_inside_new_cooldown", "passed":true,
        "refusal":"RefreshThrottled", "counters":counters(repeated)}),
    );

    let retained = verifier
        .verify(&input.access_token, &input.expected_tenant)
        .await
        .map_err(|_| "original_identity_after_refresh_refused")?;
    let final_counts = observe(&verifier).await?;
    if retained != warm || final_counts != refreshed || unix_now()? >= input.expires_at {
        return Err("final_identity_or_counters");
    }
    checks.push(
        json!({"case":"original_identity_after_unknown_refresh", "passed":true,
        "counters":counters(final_counts)}),
    );
    Ok(())
}

fn run(started: StdInstant, report: &mut Value) -> Result<(), &'static str> {
    if std::env::args_os().len() != 1 {
        return Err("arguments_forbidden");
    }
    let bytes = read_pipe()?;
    let input = parse_input(&bytes, unix_now()?)?;
    drop(bytes);
    report["run_id"] = json!(input.run_id);
    report["declared_source_revisions"] = json!({"consumer":input.declared_consumer_revision,
        "provider":input.declared_provider_revision, "attested_by_this_process":false});
    report["disk_executable_sha256"] = json!(executable_digest()?);
    let remaining = RUN_DEADLINE
        .checked_sub(started.elapsed())
        .ok_or("run_deadline")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "runtime_unavailable")?;
    let mut checks = Vec::new();
    let result = runtime.block_on(async {
        timeout(remaining, campaign(&input, &mut checks))
            .await
            .map_err(|_| "run_deadline")?
    });
    // A platform DNS lookup may outlive its canceled async request. Do not let
    // runtime Drop wait indefinitely for that blocking task; main exits after
    // emitting the fixed, redacted result and closes all process transports.
    runtime.shutdown_timeout(Duration::ZERO);
    report["checks"] = json!(checks);
    result
}

fn main() {
    let started = StdInstant::now();
    arm_watchdog(started + RUN_DEADLINE);
    let mut report = json!({"schema_version":1, "passed":false,
        "scope":"canonical native verifier against hosted access JWKS; no callback, membership, session, SMTP, KMS custody or key-rotation acceptance",
        "process_id":std::process::id(), "os":std::env::consts::OS,
        "architecture":std::env::consts::ARCH,
        "compiled_verifier_source_sha256":sha256(include_bytes!("../src/lib.rs")),
        "compiled_campaign_source_sha256":sha256(include_bytes!("hosted_key_qualification.rs")),
        "checks":[]});
    let result = run(started, &mut report);
    report["elapsed_milliseconds"] = json!(started.elapsed().as_millis());
    report["passed"] = json!(result.is_ok());
    if let Err(code) = result {
        report["failure_code"] = json!(code);
    }
    // Explicit exit prevents a withheld pipe EOF from retaining the reader
    // thread. No credential files or resources are created by this process.
    let output = serde_json::to_vec(&report).unwrap_or_default();
    let mut stdout = std::io::stdout().lock();
    let written = stdout
        .write_all(&output)
        .and_then(|_| stdout.write_all(b"\n"));
    let flushed = stdout.flush();
    std::process::exit(if result.is_ok() && written.is_ok() && flushed.is_ok() {
        0
    } else {
        1
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Value {
        json!({"schema_version":1,"synthetic_tenant":true,"run_id":"local-boundary",
            "declared_consumer_revision":"a".repeat(40),"declared_provider_revision":"b".repeat(40),
            "expected_tenant":"ten_00000000-0000-4000-8000-000000000001",
            "expected_subject":"usr_00000000-0000-4000-8000-000000000001",
            "issuer":"gatewasm","audience":"gatewasm-app",
            "jwks_url":"https://auth-staging.getforma.dev/auth/jwks",
            "max_access_lifetime_seconds":900,"expires_at":1120,"access_token":"private-marker"})
    }

    #[test]
    fn input_cannot_redirect_credentials_or_weaken_hosted_contract() {
        let base = input();
        assert!(parse_input(&serde_json::to_vec(&base).unwrap(), 1000).is_ok());
        for (field, value) in [
            ("jwks_url", json!("http://127.0.0.1/auth/jwks")),
            (
                "jwks_url",
                json!("https://auth-staging.getforma.dev/ssf/jwks"),
            ),
            (
                "jwks_url",
                json!("https://auth-staging.getforma.dev/auth/jwks?secret=x"),
            ),
            ("synthetic_tenant", json!(false)),
            ("expires_at", json!(1089)),
            ("expires_at", json!(1181)),
            ("max_access_lifetime_seconds", json!(901)),
            (
                "expected_tenant",
                json!("00000000-0000-4000-8000-000000000001"),
            ),
            ("run_id", json!("private\nmarker")),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            assert!(parse_input(&serde_json::to_vec(&changed).unwrap(), 1000).is_err());
        }
        let duplicate =
            serde_json::to_string(&base)
                .unwrap()
                .replacen('{', "{\"schema_version\":1,", 1);
        assert!(parse_input(duplicate.as_bytes(), 1000).is_err());
        let mut unknown = base;
        unknown["override_clock"] = json!(1000);
        assert!(parse_input(&serde_json::to_vec(&unknown).unwrap(), 1000).is_err());
    }

    #[test]
    fn unknown_control_preserves_claims_and_signature_without_becoming_valid() {
        let original = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(r#"{"alg":"EdDSA","typ":"JWT","kid":"known"}"#),
            URL_SAFE_NO_PAD.encode("signed-claims"),
            URL_SAFE_NO_PAD.encode([7; 64])
        );
        let control = control_token(&original, "known").unwrap();
        let before: Vec<_> = original.split('.').collect();
        let after: Vec<_> = control.split('.').collect();
        assert_eq!(before[1..], after[1..]);
        assert_ne!(before[0], after[0]);
        let header: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(after[0]).unwrap()).unwrap();
        assert_eq!(header["alg"], "EdDSA");
        assert_eq!(header["typ"], "JWT");
        assert_eq!(
            URL_SAFE_NO_PAD
                .decode(header["kid"].as_str().unwrap())
                .unwrap()
                .len(),
            8
        );
    }
}
