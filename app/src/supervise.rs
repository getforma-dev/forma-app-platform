//! Confined child processes (ADR-009 D9.1, D9.2; ADR-007 D7.10).
//!
//! A child starts with an empty environment and only its three pipes, under
//! `RLIMIT_NOFILE` and `RLIMIT_CORE` from its `Confinement` and nothing else
//! (no `RLIMIT_NPROC`, no `RLIMIT_AS`). Its resident set is sampled every
//! 100 ms; crossing the RSS line, the deadline or an output bound kills it.
//! A kill is never a partial value: the caller receives `Killed`, not output.
use std::{process::ExitStatus, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    task::JoinHandle,
};

const MIB: usize = 1024 * 1024;
/// The guest's 128 MiB output cap plus its JSON document envelope.
pub const STDOUT_LIMIT_BYTES: usize = 129 * MIB;
/// One report line is a few hundred bytes; anything near this is a fault.
pub const STDERR_LIMIT_BYTES: usize = MIB;
pub const RSS_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

pub struct Confinement {
    pub rss_limit_bytes: u64,
    pub deadline: Duration,
    pub nofile: u64,
    pub core: u64,
}

#[derive(Debug)]
pub struct ConfinedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub peak_rss_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Killed {
    /// The deadline passed; the child was killed.
    Deadline,
    /// The RSS line or an output bound was crossed; the child was killed.
    ResourceLimit,
    /// The child could not start, or ended on a signal that was not ours.
    Crashed,
    /// Reading the caller's input failed; the child was killed so a truncated
    /// input can never complete as if it were whole.
    InputFailed,
}

enum Input {
    Complete,
    ChildClosed,
    ReadFailed,
}

async fn feed(mut input: impl AsyncRead + Unpin, mut pipe: ChildStdin) -> Input {
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = match input.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return Input::ReadFailed,
        };
        if pipe.write_all(&buffer[..n]).await.is_err() {
            return Input::ChildClosed;
        }
    }
    // Closing the pipe is the child's EOF.
    match pipe.shutdown().await {
        Ok(()) => Input::Complete,
        Err(_) => Input::ChildClosed,
    }
}

async fn drain(mut pipe: impl AsyncRead + Unpin, limit: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        match pipe.read(&mut buffer).await {
            Ok(0) => return Some(bytes),
            Ok(n) if bytes.len() + n <= limit => bytes.extend_from_slice(&buffer[..n]),
            _ => return None,
        }
    }
}

#[cfg(target_os = "linux")]
fn rss_bytes(pid: u32) -> Option<u64> {
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: sysconf reads a constant system value.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    u64::try_from(page).ok().map(|page| pages * page)
}

#[cfg(target_os = "macos")]
fn rss_bytes(pid: u32) -> Option<u64> {
    // SAFETY: proc_taskinfo is plain data; proc_pidinfo writes at most `size`
    // bytes into it and reports how many it wrote.
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
    let written = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            (&mut info as *mut libc::proc_taskinfo).cast(),
            size,
        )
    };
    (written == size).then_some(info.pti_resident_size)
}

async fn kill(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

/// Run `command` confined, streaming `stdin` to it, and return its exit
/// status and bounded output; any kill returns `Killed` and no output.
pub async fn run_confined(
    mut command: Command,
    stdin: impl AsyncRead + Unpin + Send + 'static,
    confinement: &Confinement,
) -> Result<ConfinedOutput, Killed> {
    let (nofile, core) = (confinement.nofile, confinement.core);
    command
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // SAFETY: runs in the forked child before exec; setrlimit is
    // async-signal-safe and touches no memory shared with the parent.
    unsafe {
        command.pre_exec(move || {
            for (resource, value) in [(libc::RLIMIT_NOFILE, nofile), (libc::RLIMIT_CORE, core)] {
                let limit = libc::rlimit {
                    rlim_cur: value as libc::rlim_t,
                    rlim_max: value as libc::rlim_t,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let deadline = tokio::time::Instant::now() + confinement.deadline;
    let mut child = command.spawn().map_err(|_| Killed::Crashed)?;
    let pid = child.id().ok_or(Killed::Crashed)?;
    let mut writer: JoinHandle<Input> =
        tokio::spawn(feed(stdin, child.stdin.take().ok_or(Killed::Crashed)?));
    let mut stdout: JoinHandle<Option<Vec<u8>>> = tokio::spawn(drain(
        child.stdout.take().ok_or(Killed::Crashed)?,
        STDOUT_LIMIT_BYTES,
    ));
    let mut stderr: JoinHandle<Option<Vec<u8>>> = tokio::spawn(drain(
        child.stderr.take().ok_or(Killed::Crashed)?,
        STDERR_LIMIT_BYTES,
    ));
    let (mut out, mut err, mut fed) = (None, None, None);
    let mut ticker = tokio::time::interval(RSS_SAMPLE_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut peak_rss_bytes = 0u64;
    let status = loop {
        tokio::select! {
            status = child.wait() => break status.map_err(|_| Killed::Crashed)?,
            _ = ticker.tick() => {
                if let Some(rss) = rss_bytes(pid) {
                    peak_rss_bytes = peak_rss_bytes.max(rss);
                    if rss > confinement.rss_limit_bytes {
                        kill(&mut child).await;
                        return Err(Killed::ResourceLimit);
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                kill(&mut child).await;
                return Err(Killed::Deadline);
            }
            result = &mut writer, if fed.is_none() => {
                let input = result.unwrap_or(Input::ReadFailed);
                if matches!(input, Input::ReadFailed) {
                    kill(&mut child).await;
                    return Err(Killed::InputFailed);
                }
                fed = Some(input);
            }
            result = &mut stdout, if out.is_none() => match result.ok().flatten() {
                Some(bytes) => out = Some(bytes),
                None => {
                    kill(&mut child).await;
                    return Err(Killed::ResourceLimit);
                }
            },
            result = &mut stderr, if err.is_none() => match result.ok().flatten() {
                Some(bytes) => err = Some(bytes),
                None => {
                    kill(&mut child).await;
                    return Err(Killed::ResourceLimit);
                }
            },
        }
    };
    // The child is gone. Its input reached EOF (so the writer has finished)
    // or the child stopped reading early; an unfinished writer can only be
    // waiting on the caller's input and is abandoned.
    if fed.is_none() {
        if writer.is_finished() {
            if matches!(writer.await, Ok(Input::ReadFailed) | Err(_)) {
                return Err(Killed::InputFailed);
            }
        } else {
            writer.abort();
        }
    }
    // A descendant holding a pipe open cannot extend the deadline.
    let rest = async {
        let out = match out {
            Some(bytes) => Some(bytes),
            None => stdout.await.ok().flatten(),
        };
        let err = match err {
            Some(bytes) => Some(bytes),
            None => stderr.await.ok().flatten(),
        };
        (out, err)
    };
    let (out, err) = tokio::time::timeout_at(deadline, rest)
        .await
        .map_err(|_| Killed::Deadline)?;
    let (Some(stdout), Some(stderr)) = (out, err) else {
        return Err(Killed::ResourceLimit);
    };
    if status.code().is_none() {
        return Err(Killed::Crashed);
    }
    Ok(ConfinedOutput {
        status,
        stdout,
        stderr,
        peak_rss_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tokio::process::Command;

    const MIB: u64 = 1024 * 1024;

    fn confinement(rss_limit_bytes: u64, deadline: Duration) -> Confinement {
        Confinement {
            rss_limit_bytes,
            deadline,
            nofile: 64,
            core: 0,
        }
    }
    fn empty() -> &'static [u8] {
        b""
    }

    #[tokio::test]
    async fn killed_at_rss_line_reports_resource_limit() {
        // Perl holds a 256 MiB string resident until it is killed.
        let mut command = Command::new("/usr/bin/perl");
        command.args(["-e", "my $x = 'a' x (256 * 1024 * 1024); sleep 30;"]);
        let started = Instant::now();
        let outcome = run_confined(
            command,
            empty(),
            &confinement(64 * MIB, Duration::from_secs(20)),
        )
        .await;
        assert_eq!(outcome.unwrap_err(), Killed::ResourceLimit);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn killed_at_deadline_reports_deadline() {
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let started = Instant::now();
        let outcome = run_confined(
            command,
            empty(),
            &confinement(512 * MIB, Duration::from_millis(300)),
        )
        .await;
        assert_eq!(outcome.unwrap_err(), Killed::Deadline);
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(300), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }

    #[tokio::test]
    async fn child_environment_is_empty() {
        let mut command = Command::new("/usr/bin/env");
        // Even a variable the caller set explicitly does not reach the child.
        command.env("GENUI_SECRET_PROBE", "must-not-leak");
        let output = run_confined(
            command,
            empty(),
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await
        .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    }

    #[tokio::test]
    async fn child_limits_are_nofile_and_core_only_from_confinement() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "ulimit -n; ulimit -c"]);
        let output = run_confined(
            command,
            empty(),
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await
        .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "64\n0\n");
    }

    #[tokio::test]
    async fn stdin_streams_to_the_child_and_stdout_returns() {
        let input: Vec<u8> = (0..3 * MIB as usize).map(|i| (i % 251) as u8).collect();
        let output = run_confined(
            Command::new("/bin/cat"),
            std::io::Cursor::new(input.clone()),
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, input);
        assert!(output.peak_rss_bytes > 0);
    }

    #[tokio::test]
    async fn child_that_stops_reading_early_is_not_a_failure() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exec 0<&-; echo done"]);
        let output = run_confined(
            command,
            std::io::Cursor::new(vec![0u8; 8 * MIB as usize]),
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"done\n");
    }

    struct FailingInput;
    impl AsyncRead for FailingInput {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::ErrorKind::ConnectionReset.into()))
        }
    }

    #[tokio::test]
    async fn input_read_failure_kills_and_never_completes() {
        // `cat` would happily exit 0 on the truncated stream it saw.
        let input = std::io::Cursor::new(b"a,b\n1,2\n".to_vec()).chain(FailingInput);
        let outcome = run_confined(
            Command::new("/bin/cat"),
            input,
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await;
        assert_eq!(outcome.unwrap_err(), Killed::InputFailed);
    }

    #[tokio::test]
    async fn signal_exit_reports_crashed() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "kill -SEGV $$"]);
        let outcome = run_confined(
            command,
            empty(),
            &confinement(512 * MIB, Duration::from_secs(10)),
        )
        .await;
        assert_eq!(outcome.unwrap_err(), Killed::Crashed);
    }

    #[tokio::test]
    async fn stderr_beyond_its_bound_is_a_resource_limit() {
        let mut command = Command::new("/usr/bin/perl");
        command.args(["-e", "print STDERR 'x' x (2 * 1024 * 1024); sleep 30;"]);
        let outcome = run_confined(
            command,
            empty(),
            &confinement(512 * MIB, Duration::from_secs(20)),
        )
        .await;
        assert_eq!(outcome.unwrap_err(), Killed::ResourceLimit);
    }
}
