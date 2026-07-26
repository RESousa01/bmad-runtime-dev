//! Bounded child-process execution (ADR-0009).
//!
//! ADR-0001 excluded this outright; ADR-0009 admits it behind consent. The
//! bounds here are the part that does not depend on the user answering a prompt
//! correctly, so they are structural rather than advisory:
//!
//! * the working directory is pinned inside the granted workspace and is never
//!   taken from the model;
//! * a wall-clock timeout kills the child rather than waiting on it;
//! * captured output is capped, and truncation is reported rather than hidden;
//! * the command is passed as one string to the platform shell, so what the
//!   user approved in the prompt is exactly what is spawned.
//!
//! What this deliberately does *not* do is sandbox. A command runs with the
//! user's own privileges, which is what makes it useful and is why the consent
//! prompt shows the literal command line.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Wall-clock ceiling for one command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// Captured bytes per stream. A command that prints more is truncated, and the
/// result says so — a silent cut would let a model draw a conclusion from an
/// output it never fully saw.
pub const MAX_CAPTURED_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ShellError {
    #[error("the command is empty or not valid text")]
    InvalidCommand,
    #[error("the working directory is outside the granted workspace")]
    DirectoryEscape,
    #[error("the command could not be started")]
    SpawnFailed,
    #[error("the command exceeded its time limit and was terminated")]
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutcome {
    /// `None` when the child was killed by a signal or the timeout.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub duration_ms: u64,
}

/// What the caller asks for. The workspace root is supplied by the host, never
/// by the model — a model-chosen root would make the pin meaningless.
#[derive(Clone, Debug)]
pub struct ShellRequest<'a> {
    pub command: &'a str,
    pub workspace_root: &'a Path,
    /// Optional subdirectory, relative to the root. Rejected if it escapes.
    pub relative_dir: Option<&'a str>,
    pub timeout: Duration,
}

/// Resolves the working directory, refusing anything that leaves the root.
///
/// `..` and absolute paths are rejected before touching the filesystem, and the
/// canonicalised result is re-checked against the canonicalised root so a
/// symlink cannot walk out either.
fn resolve_dir(root: &Path, relative: Option<&str>) -> Result<PathBuf, ShellError> {
    let Some(relative) = relative.map(str::trim).filter(|value| !value.is_empty()) else {
        return root.canonicalize().map_err(|_| ShellError::DirectoryEscape);
    };
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ShellError::DirectoryEscape);
    }
    let root = root
        .canonicalize()
        .map_err(|_| ShellError::DirectoryEscape)?;
    let resolved = root
        .join(candidate)
        .canonicalize()
        .map_err(|_| ShellError::DirectoryEscape)?;
    if !resolved.starts_with(&root) {
        return Err(ShellError::DirectoryEscape);
    }
    Ok(resolved)
}

/// Reads at most `MAX_CAPTURED_BYTES`, reporting whether more was available.
fn read_capped(mut stream: impl std::io::Read) -> (String, bool) {
    let mut buffer = Vec::new();
    // One byte past the cap, so "exactly at the cap" is not misreported as
    // truncated.
    let mut limited = (&mut stream).take(MAX_CAPTURED_BYTES as u64 + 1);
    let _ = limited.read_to_end(&mut buffer);
    let truncated = buffer.len() > MAX_CAPTURED_BYTES;
    buffer.truncate(MAX_CAPTURED_BYTES);
    (String::from_utf8_lossy(&buffer).into_owned(), truncated)
}

/// Runs one command to completion, or kills it at the timeout.
///
/// # Errors
///
/// Returns [`ShellError`] when the command is empty, the directory escapes the
/// workspace, the child cannot be spawned, or the timeout elapses.
pub fn run(request: ShellRequest<'_>) -> Result<ShellOutcome, ShellError> {
    let command = request.command.trim();
    if command.is_empty() {
        return Err(ShellError::InvalidCommand);
    }
    let directory = resolve_dir(request.workspace_root, request.relative_dir)?;

    // One string to the platform shell, so the spawned command is textually the
    // one shown in the consent prompt. Splitting and re-quoting here would mean
    // approving one thing and running another.
    let mut child = shell_command(command)
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ShellError::SpawnFailed)?;

    // Drain both pipes on their own threads *while* the child runs. Reading
    // only after it exits deadlocks the moment output exceeds the pipe buffer:
    // the child blocks writing, nobody is draining, and the command dies at the
    // timeout having produced nothing.
    let stdout_reader = child
        .stdout
        .take()
        .map(|stream| std::thread::spawn(move || read_capped(stream)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stream| std::thread::spawn(move || read_capped(stream)));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        if started.elapsed() >= request.timeout {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    // Join after the child has exited or been killed, so the readers always see
    // EOF and can never outlive the call.
    let (stdout, stdout_truncated) = stdout_reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_else(|| (String::new(), false));
    let (stderr, stderr_truncated) = stderr_reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_else(|| (String::new(), false));

    if timed_out {
        return Err(ShellError::TimedOut);
    }

    Ok(ShellOutcome {
        exit_code: status.and_then(|status| status.code()),
        stdout,
        stderr,
        truncated: stdout_truncated || stderr_truncated,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut builder = Command::new("cmd");
    builder.arg("/C").arg(command);
    builder
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut builder = Command::new("sh");
    builder.arg("-c").arg(command);
    builder
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{run, ShellError, ShellRequest, MAX_CAPTURED_BYTES};
    use std::time::Duration;

    fn request<'a>(command: &'a str, root: &'a std::path::Path) -> ShellRequest<'a> {
        ShellRequest {
            command,
            workspace_root: root,
            relative_dir: None,
            timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn a_command_runs_and_reports_its_output_and_code() {
        let directory = tempfile::tempdir().expect("temp dir");
        let outcome = run(request("echo hello", directory.path())).expect("ran");
        assert_eq!(outcome.exit_code, Some(0));
        assert!(outcome.stdout.contains("hello"));
        assert!(!outcome.truncated);
    }

    #[test]
    fn a_failing_command_reports_its_code_rather_than_erroring() {
        let directory = tempfile::tempdir().expect("temp dir");
        // A non-zero exit is information for the model, not a host failure.
        let outcome = run(request("exit 3", directory.path())).expect("ran");
        assert_eq!(outcome.exit_code, Some(3));
    }

    #[test]
    fn an_empty_command_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            run(request("   ", directory.path())),
            Err(ShellError::InvalidCommand)
        );
    }

    /// The working directory is the one bound the user cannot get wrong by
    /// answering a prompt carelessly, so it is enforced structurally.
    #[test]
    fn the_working_directory_cannot_escape_the_workspace() {
        let directory = tempfile::tempdir().expect("temp dir");
        for escape in ["..", "../..", "sub/../.."] {
            let outcome = run(ShellRequest {
                relative_dir: Some(escape),
                ..request("echo x", directory.path())
            });
            assert_eq!(
                outcome,
                Err(ShellError::DirectoryEscape),
                "{escape} must not resolve outside the workspace",
            );
        }
        #[cfg(windows)]
        let absolute = "C:/Windows";
        #[cfg(not(windows))]
        let absolute = "/etc";
        assert_eq!(
            run(ShellRequest {
                relative_dir: Some(absolute),
                ..request("echo x", directory.path())
            }),
            Err(ShellError::DirectoryEscape),
        );
    }

    #[test]
    fn a_subdirectory_inside_the_workspace_is_allowed() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(directory.path().join("sub")).expect("subdir");
        let outcome = run(ShellRequest {
            relative_dir: Some("sub"),
            ..request("echo inside", directory.path())
        })
        .expect("ran");
        assert_eq!(outcome.exit_code, Some(0));
    }

    #[test]
    fn a_command_that_overruns_its_deadline_is_killed() {
        let directory = tempfile::tempdir().expect("temp dir");
        #[cfg(windows)]
        let sleeper = "ping -n 30 127.0.0.1 > nul";
        #[cfg(not(windows))]
        let sleeper = "sleep 30";
        let outcome = run(ShellRequest {
            timeout: Duration::from_millis(300),
            ..request(sleeper, directory.path())
        });
        assert_eq!(outcome, Err(ShellError::TimedOut));
    }

    /// Truncation must be visible: a model that reasons from a silently cut
    /// output is reasoning from something it never saw.
    #[test]
    fn oversized_output_is_capped_and_says_so() {
        let directory = tempfile::tempdir().expect("temp dir");
        let script = directory.path().join("noisy.txt");
        std::fs::write(&script, "x".repeat(MAX_CAPTURED_BYTES * 2)).expect("write");
        #[cfg(windows)]
        let command = "type noisy.txt";
        #[cfg(not(windows))]
        let command = "cat noisy.txt";
        let outcome = run(request(command, directory.path())).expect("ran");
        assert!(outcome.truncated, "the cap must be reported");
        assert!(outcome.stdout.len() <= MAX_CAPTURED_BYTES);
    }
}
