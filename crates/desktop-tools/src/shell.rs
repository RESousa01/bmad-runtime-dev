//! Bounded child-process execution (ADR-0009).
//!
//! ADR-0001 excluded this outright; ADR-0009 admits it behind consent. The
//! bounds here are the part that does not depend on the user answering a prompt
//! correctly, so they are structural rather than advisory:
//!
//! * the working directory is pinned inside the granted workspace and is never
//!   taken from the model;
//! * a wall-clock timeout bounds the whole call, and kills the child *tree*
//!   rather than waiting on it — see "Bounding the call" below;
//! * captured output is capped, and truncation is reported rather than hidden;
//! * the command is a single line passed to the platform shell, so what the
//!   user approved in the prompt is exactly what is spawned.
//!
//! # Bounding the call
//!
//! Killing the process we spawned is not the same as bounding the call, and an
//! earlier revision confused the two. `cmd.exe` is what we spawn; the thing
//! that actually does the work — `node.exe`, a dev server, a test runner — is
//! its child, and it inherits the *write* ends of our stdout and stderr pipes.
//! `Child::kill` terminates `cmd.exe` alone, so those write handles stay open,
//! the capture threads never see EOF, and joining them blocked for as long as
//! the grandchild felt like living. The certifying test asked for a 300 ms
//! deadline and took 29.3 s to return, and would have taken longer for a
//! longer-lived grandchild. Three things now hold the bound, in order of how
//! much they can be relied on:
//!
//! 1. **The call never waits on a pipe it does not control.** On the timeout
//!    path the capture threads are abandoned, not joined; on the success path
//!    they are collected with [`DRAIN_GRACE`] and a capture that does not
//!    finish is reported as truncated. This is platform-independent and holds
//!    even if everything below fails, so it is what makes the deadline a bound
//!    rather than a hope.
//! 2. **On Windows the tree dies with the call.** The child is confined to a
//!    Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so closing our
//!    handle terminates every process still inside it. Without this, a call the
//!    product reported as "timed out" would leave a grandchild running with the
//!    user's full privileges, which is a worse outcome than the hang.
//! 3. **`Child::kill` remains**, as the fallback for a job object that could
//!    not be created and for platforms that have none.
//!
//! Two consequences are deliberate and worth knowing before calling this:
//!
//! * On Windows a command **cannot leave a process running after the call
//!   returns**. `start /b some-server` is killed when `run` returns, on the
//!   success path as much as the timeout path. This is not a primitive for
//!   starting a long-lived daemon, and quietly leaking one out of a call
//!   advertised as bounded is precisely what point 2 exists to stop.
//! * Off Windows there is no process-group equivalent reachable without a new
//!   dependency, so a backgrounded grandchild survives. The *call* is still
//!   bounded by point 1; the process is not. The product ships on Windows, so
//!   this is a known gap rather than an accepted one.
//!
//! What this deliberately does *not* do is sandbox. A command runs with the
//! user's own privileges, which is what makes it useful and is why the consent
//! prompt shows the literal command line. It also means an approved command can
//! read files the workspace broker would refuse, and can write without the
//! governed-IO checkpoint — which is why [`crate::permission`] treats a session
//! grant over a command prefix as the dangerous primitive it is.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Wall-clock ceiling for one command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_mins(2);
/// Captured bytes per stream. A command that prints more is truncated, and the
/// result says so — a silent cut would let a model draw a conclusion from an
/// output it never fully saw.
pub const MAX_CAPTURED_BYTES: usize = 64 * 1024;
/// How long the capture threads get to finish once the child is gone.
///
/// Everything the child itself wrote is already sitting in the pipe buffer by
/// then and is read without waiting, so this is only ever spent on a *pipe
/// someone else still holds open*. It is a ceiling, not a delay: the ordinary
/// case returns the moment the readers see EOF.
const DRAIN_GRACE: Duration = Duration::from_millis(500);
/// How often the wait loop re-checks the child and the deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ShellError {
    #[error("the command is empty, spans more than one line, or is not valid text")]
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

/// Reads a stream to EOF, keeping at most `MAX_CAPTURED_BYTES`.
///
/// It keeps reading past the cap and discards the excess rather than closing
/// the pipe. Dropping the reader early hands the child a broken pipe, so a
/// command that merely printed a lot — a passing test suite, say — would come
/// back looking as though it had failed. The exit code has to keep meaning
/// what it says.
fn read_capped(mut stream: impl std::io::Read) -> (String, bool) {
    let mut kept: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 8192];
    let mut truncated = false;
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let remaining = MAX_CAPTURED_BYTES.saturating_sub(kept.len());
                if remaining == 0 {
                    truncated = true;
                } else if read > remaining {
                    kept.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                } else {
                    kept.extend_from_slice(&chunk[..read]);
                }
            }
        }
    }
    (String::from_utf8_lossy(&kept).into_owned(), truncated)
}

/// Starts draining one stream on its own thread and hands back the result slot.
///
/// A channel rather than a [`std::thread::JoinHandle`] because the join has no
/// deadline: the caller must be able to give up on a reader whose pipe some
/// other process is holding open, and a `JoinHandle` offers no way to.
fn spawn_capture<R>(stream: R) -> Receiver<(String, bool)>
where
    R: std::io::Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    let _ = std::thread::spawn(move || {
        // A send failure means the caller already gave up and returned; there
        // is nobody left to tell.
        let _ = sender.send(read_capped(stream));
    });
    receiver
}

/// Collects one drained stream, waiting no longer than `grace`.
///
/// A capture that does not finish in time is reported as truncated. Returning
/// an empty string on its own would let a model conclude that a command printed
/// nothing, which is the same class of mistake as a silent cap.
fn collect_capture(receiver: Option<Receiver<(String, bool)>>, grace: Duration) -> (String, bool) {
    receiver.map_or_else(
        || (String::new(), false),
        |receiver| {
            receiver
                .recv_timeout(grace)
                .unwrap_or((String::new(), true))
        },
    )
}

/// Whether a command carries a control character that would let one approved
/// line become several commands.
///
/// A prompt renders one line; the platform shell will happily run everything a
/// line break chains onto it.
#[must_use]
pub fn contains_control_characters(command: &str) -> bool {
    command.chars().any(|c| c == '\n' || c == '\r' || c == '\0')
}

/// Runs one command to completion, or kills it at the timeout.
///
/// # Errors
///
/// Returns [`ShellError`] when the command is empty or multi-line, the
/// directory escapes the workspace, the child cannot be spawned, or the timeout
/// elapses.
pub fn run(request: &ShellRequest<'_>) -> Result<ShellOutcome, ShellError> {
    let command = request.command.trim();
    if command.is_empty() || contains_control_characters(command) {
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

    // Confine the child *and everything it starts* before it has had time to
    // start anything. `Child::kill` reaches the shell only; this is what makes
    // the tree die with the call. See the module doc for what it still misses.
    let process_tree = ProcessTree::confine(&child);

    // Drain both pipes on their own threads *while* the child runs. Reading
    // only after it exits deadlocks the moment output exceeds the pipe buffer:
    // the child blocks writing, nobody is draining, and the command dies at the
    // timeout having produced nothing.
    let stdout_reader = child.stdout.take().map(spawn_capture);
    let stderr_reader = child.stderr.take().map(spawn_capture);

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        if started.elapsed() >= request.timeout {
            timed_out = true;
            break None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    // Closing the job terminates every process still inside it, so a
    // grandchild cannot outlive the call holding our pipes — or the user's
    // privileges. `kill` stays as the fallback for a job we could not create
    // and for platforms that have none.
    drop(process_tree);
    let _ = child.kill();
    let _ = child.wait();

    if timed_out {
        // Deliberately *not* collecting the captures. Whether a reader ever
        // sees EOF depends on processes this call does not control, so waiting
        // on one here is what turned the deadline into a suggestion. The
        // outcome is discarded on this path anyway; an abandoned reader ends
        // when its pipe closes and holds at most `MAX_CAPTURED_BYTES`.
        return Err(ShellError::TimedOut);
    }

    let (stdout, stdout_truncated) = collect_capture(stdout_reader, DRAIN_GRACE);
    let (stderr, stderr_truncated) = collect_capture(stderr_reader, DRAIN_GRACE);

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

/// Ownership of a spawned command's whole process tree, where the platform can
/// express one. Dropping it ends the tree.
///
/// Off Windows there is no equivalent that does not pull in a new dependency
/// (a process group needs `killpg`, so `libc`), so [`Self::confine`] returns
/// `None` and only the bounds that do not depend on this apply. The module doc
/// records that gap.
#[cfg(not(windows))]
struct ProcessTree;

#[cfg(not(windows))]
impl ProcessTree {
    fn confine(_child: &std::process::Child) -> Option<Self> {
        None
    }
}

#[cfg(windows)]
use windows_job::ProcessTree;

/// A Windows Job Object holding the command's process tree.
///
/// The interesting flag is `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: when the last
/// handle to the job closes, Windows terminates every process still inside it.
/// That makes the kill survive our own failure modes — a panic unwinding out of
/// `run`, or a `kill` that raced the child's exit — rather than depending on
/// the happy path reaching a cleanup call.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub(super) struct ProcessTree(HANDLE);

    impl ProcessTree {
        /// Puts `child` — and everything it goes on to start — in a job that
        /// dies with this value.
        ///
        /// Returns `None` if the job cannot be created, configured, or
        /// assigned. A caller that gets `None` is no worse off than before this
        /// existed, which is why every step degrades rather than failing the
        /// command: refusing to run because the kernel would not hand out a job
        /// object trades a real capability for a hypothetical one.
        ///
        /// **Residual race.** The child is assigned just after `CreateProcess`
        /// returns, not before it runs. A grandchild started in that window —
        /// microseconds, before `cmd.exe` has parsed its own command line — is
        /// outside the job and survives. Closing it needs `CREATE_SUSPENDED`
        /// plus a `ResumeThread` on a thread handle `std::process` does not
        /// expose, so it would mean spawning the child by hand and giving up
        /// everything std does correctly around handle inheritance.
        pub(super) fn confine(child: &Child) -> Option<Self> {
            // SAFETY: a null name and null attributes are the documented
            // "unnamed, default-security job object" arguments.
            let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.ok()?;
            let tree = Self(job);

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `limits` outlives the call, and the length passed is the
            // size of exactly the type the information class names.
            let configured = unsafe {
                SetInformationJobObject(
                    tree.0,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).ok()?,
                )
            };
            configured.ok()?;

            // Without the flag above this assignment would be worse than
            // nothing: the tree would be tracked and never killed. `tree` is
            // dropped on the `?` if the flag did not take, closing the job.
            let process = HANDLE(child.as_raw_handle());
            // SAFETY: both handles are open and owned — `child` outlives this
            // call, and `tree` owns the job handle.
            unsafe { AssignProcessToJobObject(tree.0, process) }.ok()?;
            Some(tree)
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            // Closing the last handle is what terminates the tree.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{run, ShellError, ShellRequest, MAX_CAPTURED_BYTES};
    use std::time::{Duration, Instant};

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
        let outcome = run(&request("echo hello", directory.path())).expect("ran");
        assert_eq!(outcome.exit_code, Some(0));
        assert!(outcome.stdout.contains("hello"));
        assert!(!outcome.truncated);
    }

    #[test]
    fn a_failing_command_reports_its_code_rather_than_erroring() {
        let directory = tempfile::tempdir().expect("temp dir");
        // A non-zero exit is information for the model, not a host failure.
        let outcome = run(&request("exit 3", directory.path())).expect("ran");
        assert_eq!(outcome.exit_code, Some(3));
    }

    #[test]
    fn an_empty_command_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            run(&request("   ", directory.path())),
            Err(ShellError::InvalidCommand)
        );
    }

    /// The prompt shows one line; the shell would run all of them. Approving
    /// `echo hi` and getting a second command as well is exactly the
    /// approval-laundering this module exists to prevent.
    #[test]
    fn a_multi_line_command_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        let smuggled = [
            format!("echo hi{}echo surprise", '\n'),
            format!("echo hi{}{}del important.txt", '\r', '\n'),
            format!("echo hi{}echo surprise", '\0'),
        ];
        for command in smuggled {
            assert_eq!(
                run(&request(&command, directory.path())),
                Err(ShellError::InvalidCommand),
                "a command carrying a line break must never reach the shell",
            );
        }
    }

    /// The working directory is the one bound the user cannot get wrong by
    /// answering a prompt carelessly, so it is enforced structurally.
    #[test]
    fn the_working_directory_cannot_escape_the_workspace() {
        let directory = tempfile::tempdir().expect("temp dir");
        for escape in ["..", "../..", "sub/../.."] {
            assert_eq!(
                run(&ShellRequest {
                    relative_dir: Some(escape),
                    ..request("echo x", directory.path())
                }),
                Err(ShellError::DirectoryEscape),
                "{escape} must not resolve outside the workspace",
            );
        }
        #[cfg(windows)]
        let absolute = "C:/Windows";
        #[cfg(not(windows))]
        let absolute = "/etc";
        assert_eq!(
            run(&ShellRequest {
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
        let outcome = run(&ShellRequest {
            relative_dir: Some("sub"),
            ..request("echo inside", directory.path())
        })
        .expect("ran");
        assert_eq!(outcome.exit_code, Some(0));
    }

    /// A shell that sleeps for `SLEEPER_SECONDS` without printing.
    ///
    /// The Windows form redirects stdout so the test is not also a truncation
    /// test; stderr stays connected, which is the handle a surviving grandchild
    /// used to hold open.
    #[cfg(windows)]
    const SLEEPER: &str = "ping -n 30 127.0.0.1 > nul";
    #[cfg(not(windows))]
    const SLEEPER: &str = "sleep 30";
    /// Long enough that a call which waited on the sleeper instead of its own
    /// deadline cannot possibly come in under [`OVERRUN_CEILING`].
    const SLEEPER_SECONDS: u64 = 30;
    const OVERRUN_DEADLINE: Duration = Duration::from_millis(300);
    /// Generous against scheduler noise and still an order of magnitude below
    /// `SLEEPER_SECONDS`, so this cannot pass by waiting for the child.
    const OVERRUN_CEILING: Duration = Duration::from_secs(5);

    /// The deadline has to bound the *call*, not merely produce the right error
    /// eventually.
    ///
    /// This asserted only `Err(TimedOut)` before, and that made it worthless:
    /// deleting the kill entirely left it green, and on Windows it was already
    /// taking 29.3 s for a 300 ms deadline. `cmd.exe` is what we spawn, `ping`
    /// is what does the waiting, and killing the former left the latter holding
    /// the stderr pipe — so the call blocked in `join` until the grandchild
    /// chose to exit. Asserting elapsed time is the whole point: a timeout test
    /// that cannot fail on a hang is testing nothing.
    #[test]
    fn a_command_that_overruns_its_deadline_is_killed_within_its_deadline() {
        let directory = tempfile::tempdir().expect("temp dir");
        let started = Instant::now();
        let outcome = run(&ShellRequest {
            timeout: OVERRUN_DEADLINE,
            ..request(SLEEPER, directory.path())
        });
        let elapsed = started.elapsed();

        assert_eq!(outcome, Err(ShellError::TimedOut));
        assert!(
            elapsed < OVERRUN_CEILING,
            "the deadline must bound the call: {OVERRUN_DEADLINE:?} deadline took {elapsed:?}, \
             which means it waited on the child ({SLEEPER_SECONDS}s) rather than the clock",
        );
    }

    /// The case the previous revision got wrong, made explicit: the thing that
    /// holds the pipe open is a *grandchild*, and `Child::kill` never reached
    /// it. One extra shell layer is the smallest faithful model of `cmd.exe`
    /// running `node.exe`.
    #[test]
    fn a_grandchild_holding_the_pipes_does_not_outlive_the_deadline() {
        let directory = tempfile::tempdir().expect("temp dir");
        #[cfg(windows)]
        let nested = format!("cmd /c {SLEEPER}");
        #[cfg(not(windows))]
        let nested = format!("sh -c '{SLEEPER}'");

        let started = Instant::now();
        let outcome = run(&ShellRequest {
            timeout: OVERRUN_DEADLINE,
            ..request(&nested, directory.path())
        });
        let elapsed = started.elapsed();

        assert_eq!(outcome, Err(ShellError::TimedOut));
        assert!(
            elapsed < OVERRUN_CEILING,
            "a grandchild must not extend the call: {OVERRUN_DEADLINE:?} deadline took {elapsed:?}",
        );
    }

    /// The deadline must kill the *tree*, not merely report that it elapsed.
    ///
    /// The two tests above are held up by *not waiting* on the readers, so they
    /// stay green with the kill removed entirely — the same hole the audit
    /// found in the old test, relocated. This one closes it: it watches for a
    /// marker that only a grandchild which outlived the call can write, so
    /// removing the tree kill fails here even though the call still returns on
    /// time. The two properties are separate and need separate tests.
    ///
    /// Windows-only because the Job Object is what is under test. Off Windows
    /// there is no tree kill reachable without a new dependency, and the module
    /// doc says so rather than a test pretending otherwise.
    #[cfg(windows)]
    #[test]
    fn the_deadline_kills_the_grandchild_and_not_only_the_shell() {
        /// Long enough for the grandchild to exist and announce itself before
        /// the deadline, so a vacuous pass is distinguishable from a real one.
        const DEADLINE: Duration = Duration::from_secs(1);
        /// Past the grandchild's own ~3s lifetime, so "the marker never
        /// appeared" means it was killed rather than merely slow.
        const WATCH_FOR: Duration = Duration::from_secs(6);

        let directory = tempfile::tempdir().expect("temp dir");
        let launched = directory.path().join("launched.txt");
        let survived = directory.path().join("survived.txt");
        // Scripts rather than one chained command line: `&`, `^` and nested
        // quotes inside a single `cmd /C` string are their own source of test
        // bugs, and a test that fails for a quoting reason certifies nothing.
        //
        // `start /b` puts the survivor in a process of its own, so it is a
        // *grandchild* — the thing `Child::kill` has never reached. It writes
        // `launched.txt` immediately and `survived.txt` only after outliving
        // the deadline, which is what tells the two failure modes apart.
        std::fs::write(
            directory.path().join("survivor.cmd"),
            "@echo off\r\n\
             echo launched > launched.txt\r\n\
             ping -n 4 127.0.0.1 > nul\r\n\
             echo survived > survived.txt\r\n",
        )
        .expect("survivor script");
        std::fs::write(
            directory.path().join("ticker.cmd"),
            "@echo off\r\nstart /b .\\survivor.cmd\r\nping -n 30 127.0.0.1 > nul\r\n",
        )
        .expect("ticker script");

        let started = Instant::now();
        // `.\` because this machine sets `NoDefaultCurrentDirectoryInExePath`,
        // and a bare name would then fail to resolve for a reason that has
        // nothing to do with what is under test.
        let outcome = run(&ShellRequest {
            timeout: DEADLINE,
            ..request(".\\ticker.cmd", directory.path())
        });
        assert_eq!(outcome, Err(ShellError::TimedOut));
        assert!(started.elapsed() < OVERRUN_CEILING);
        assert!(
            launched.exists(),
            "the grandchild never started, so this test would pass without proving \
             anything — fix the fixture rather than trusting the result",
        );

        while started.elapsed() < WATCH_FOR {
            assert!(
                !survived.exists(),
                "a grandchild outlived a call the product reported as timed out, and \
                 went on running with the user's privileges",
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// A command can succeed and still leave something holding the pipes. That
    /// path has no timeout to fall back on, so it needs its own bound — an
    /// earlier revision would have blocked here forever.
    #[test]
    fn a_successful_command_that_leaves_a_writer_behind_still_returns() {
        let directory = tempfile::tempdir().expect("temp dir");
        #[cfg(windows)]
        let backgrounded = format!("start /b cmd /c {SLEEPER}");
        #[cfg(not(windows))]
        let backgrounded = format!("{SLEEPER} &");

        let started = Instant::now();
        let outcome = run(&request(&backgrounded, directory.path()));
        let elapsed = started.elapsed();

        assert!(outcome.is_ok(), "the shell itself exited: {outcome:?}");
        assert!(
            elapsed < OVERRUN_CEILING,
            "a surviving writer must not extend the call: took {elapsed:?}",
        );
    }

    /// Truncation must be visible: a model that reasons from a silently cut
    /// output is reasoning from something it never saw.
    ///
    /// The exit code must *also* survive. An earlier revision stopped reading
    /// at the cap, which handed the child a broken pipe — so a command that
    /// merely printed a lot came back looking as though it had failed.
    #[test]
    fn oversized_output_is_capped_without_corrupting_the_exit_code() {
        let directory = tempfile::tempdir().expect("temp dir");
        let noisy = directory.path().join("noisy.txt");
        std::fs::write(&noisy, "x".repeat(MAX_CAPTURED_BYTES * 4)).expect("write");
        #[cfg(windows)]
        let command = "type noisy.txt";
        #[cfg(not(windows))]
        let command = "cat noisy.txt";
        let outcome = run(&request(command, directory.path())).expect("ran");
        assert!(outcome.truncated, "the cap must be reported");
        assert!(outcome.stdout.len() <= MAX_CAPTURED_BYTES);
        assert_eq!(
            outcome.exit_code,
            Some(0),
            "a chatty command that succeeded must not look like a failure",
        );
    }
}
