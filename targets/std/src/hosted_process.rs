//! Bounded process transport below provider-specific admission and semantics.
pub(crate) mod stream;

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) struct ProcessRequest<'a> {
    pub program: &'a Path,
    pub arguments: &'a [OsString],
    pub environment: &'a [(OsString, OsString)],
    pub stdin: &'a [u8],
    pub maximum_stdout_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub timeout: Duration,
    pub require_process_group: bool,
}
#[derive(Debug)]
pub(crate) enum ProcessError {
    InvalidRequest(&'static str),
    Unsupported(&'static str),
    Launch(io::Error),
}
#[derive(Clone, Debug)]
pub(crate) enum ProcessTerminal {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
    ProviderLost(String),
}
#[derive(Debug, Default)]
pub(crate) struct CapturedOutput {
    pub retained: Vec<u8>,
    pub observed_bytes: u64,
}
#[derive(Debug)]
pub(crate) struct ProcessReport {
    pub terminal: ProcessTerminal,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub elapsed_millis: u64,
    pub launched: bool,
}
const RETIREMENT: Duration = Duration::from_millis(100);
const POLL: Duration = Duration::from_millis(2);

pub(crate) fn run_process(
    request: &ProcessRequest<'_>,
    cancelled: impl Fn() -> bool,
) -> Result<ProcessReport, ProcessError> {
    if !request.program.is_absolute()
        || request.stdin.len() > 256
        || request.maximum_stdout_bytes > 131_116
        || request.maximum_stderr_bytes > 131_116
        || request.timeout.is_zero()
    {
        return Err(ProcessError::InvalidRequest(
            "invalid bounded process request",
        ));
    }
    if request.require_process_group && !cfg!(unix) {
        return Err(ProcessError::Unsupported(
            "process-group retirement requires Unix",
        ));
    }
    if !cfg!(any(unix, windows)) {
        return Err(ProcessError::Unsupported(
            "bounded pipe polling unavailable on this platform",
        ));
    }
    let started = Instant::now();
    if cancelled() {
        return Ok(report(
            ProcessTerminal::Cancelled,
            CapturedOutput::default(),
            CapturedOutput::default(),
            started,
            false,
        ));
    }
    let mut out = CapturedOutput {
        retained: Vec::with_capacity(request.maximum_stdout_bytes),
        observed_bytes: 0,
    };
    let mut err = CapturedOutput {
        retained: Vec::with_capacity(request.maximum_stderr_bytes),
        observed_bytes: 0,
    };
    let mut command = Command::new(request.program);
    command
        .args(request.arguments)
        .env_clear()
        .envs(request.environment.iter().map(|(k, v)| (k, v)))
        .stdin(if request.stdin.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn().map_err(ProcessError::Launch)?;
    let mut guard = ChildGuard(child, false);
    let child = &mut guard.0;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut stdin = child.stdin.take();
    let preparation = prepare_pipe(&stdout)
        .and_then(|()| prepare_pipe(&stderr))
        .and_then(|()| {
            if let Some(input) = &stdin {
                prepare_pipe(input)
            } else {
                Ok(())
            }
        });
    let mut out_eof = false;
    let mut err_eof = false;
    let mut sent = 0;
    if let Err(error) = preparation {
        terminate(child);
        let _ = child.wait();
        guard.1 = true;
        return Ok(report(
            ProcessTerminal::ProviderLost(error.to_string()),
            out,
            err,
            started,
            true,
        ));
    }
    let mut terminal = None;
    let mut retired_at = None;
    loop {
        if terminal.is_none() {
            if cancelled() {
                terminal = Some(ProcessTerminal::Cancelled);
            } else if started.elapsed() >= request.timeout {
                terminal = Some(ProcessTerminal::TimedOut);
            } else {
                match child.try_wait() {
                    Ok(Some(status)) => terminal = Some(ProcessTerminal::Exited(status)),
                    Ok(None) => {}
                    Err(error) => terminal = Some(ProcessTerminal::ProviderLost(error.to_string())),
                }
            }
        }
        if terminal.is_some() && retired_at.is_none() {
            terminate(child);
            stdin.take();
            retired_at = Some(Instant::now());
        }
        if let Some(input) = &mut stdin {
            // At most256 bytes fit an empty platform pipe; Unix is additionally
            // nonblocking. Never retry a write without returning to the deadline.
            match input.write(&request.stdin[sent..]) {
                Ok(0) => {
                    terminal = Some(ProcessTerminal::ProviderLost(
                        "process stdin closed before delivery".into(),
                    ))
                }
                Ok(size) => {
                    sent += size;
                    if sent == request.stdin.len() {
                        stdin.take();
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => {
                    terminal = Some(ProcessTerminal::ProviderLost(format!(
                        "process stdin: {error}"
                    )))
                }
            }
        }
        for result in [
            drain(
                &mut stdout,
                &mut out,
                request.maximum_stdout_bytes,
                &mut out_eof,
            ),
            drain(
                &mut stderr,
                &mut err,
                request.maximum_stderr_bytes,
                &mut err_eof,
            ),
        ] {
            if let Err(error) = result {
                record_pipe_failure(&mut terminal, error);
            }
        }
        if let Some(retired) = retired_at {
            if out_eof && err_eof {
                break;
            }
            if retired.elapsed() >= RETIREMENT {
                if matches!(terminal, Some(ProcessTerminal::Exited(_))) {
                    terminal = Some(ProcessTerminal::ProviderLost(
                        "descendant pipe did not retire within bound".into(),
                    ));
                }
                break;
            }
        }
        thread::sleep(POLL);
    }
    // Kill precedes reap; there are no pipe-reader threads to join. Dropping the
    // captured pipe handles also retires descriptors held by escaped descendants.
    let _ = child.wait();
    guard.1 = true;
    Ok(report(
        terminal.expect("terminal process"),
        out,
        err,
        started,
        true,
    ))
}
// Retirement can discover a broken pipe after cancellation or the deadline.
// That cleanup failure must not replace the already established terminal cause.
fn record_pipe_failure(terminal: &mut Option<ProcessTerminal>, error: io::Error) {
    if terminal.is_none() || matches!(terminal, Some(ProcessTerminal::Exited(_))) {
        *terminal = Some(ProcessTerminal::ProviderLost(format!(
            "process pipe: {error}"
        )));
    }
}

fn report(
    terminal: ProcessTerminal,
    stdout: CapturedOutput,
    stderr: CapturedOutput,
    started: Instant,
    launched: bool,
) -> ProcessReport {
    ProcessReport {
        terminal,
        stdout,
        stderr,
        elapsed_millis: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        launched,
    }
}
struct ChildGuard(Child, bool);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.1 {
            terminate(&mut self.0);
            let _ = self.0.wait();
        }
    }
}
fn terminate(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        // The child is spawned as leader of its own group. Also kill that group
        // after leader exit so descendants cannot retain inherited pipe writers.
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let _ = child.kill();
}
#[cfg(unix)]
fn prepare_pipe(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(unix))]
fn prepare_pipe<T>(_: &T) -> io::Result<()> {
    Ok(())
}
#[cfg(unix)]
fn available<T>(_: &T) -> io::Result<bool> {
    Ok(true)
}
#[cfg(windows)]
fn available(pipe: &impl std::os::windows::io::AsRawHandle) -> io::Result<bool> {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn PeekNamedPipe(
            handle: *mut c_void,
            buffer: *mut c_void,
            size: u32,
            read: *mut u32,
            available: *mut u32,
            remaining: *mut u32,
        ) -> i32;
    }
    let mut bytes = 0;
    let success = unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(109) {
            return Ok(true);
        }
        return Err(error);
    }
    Ok(bytes > 0)
}
#[cfg(not(any(unix, windows)))]
fn available<T>(_: &T) -> io::Result<bool> {
    Ok(false)
}
#[cfg(unix)]
trait Pipe: Read {}
#[cfg(unix)]
impl<T: Read> Pipe for T {}
#[cfg(windows)]
trait Pipe: Read + std::os::windows::io::AsRawHandle {}
#[cfg(windows)]
impl<T: Read + std::os::windows::io::AsRawHandle> Pipe for T {}
#[cfg(not(any(unix, windows)))]
trait Pipe: Read {}
#[cfg(not(any(unix, windows)))]
impl<T: Read> Pipe for T {}
fn drain(
    pipe: &mut impl Pipe,
    output: &mut CapturedOutput,
    maximum: usize,
    eof: &mut bool,
) -> io::Result<()> {
    if *eof || !available(pipe)? {
        return Ok(());
    }
    let mut buffer = [0; 4096];
    // One read per iteration bounds mandatory work even for an infinite writer.
    match pipe.read(&mut buffer) {
        Ok(0) => *eof = true,
        Ok(size) => {
            output.observed_bytes = output.observed_bytes.saturating_add(size as u64);
            let retained = size.min(maximum.saturating_sub(output.retained.len()));
            output.retained.extend_from_slice(&buffer[..retained]);
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) => {}
        #[cfg(windows)]
        Err(error) if error.raw_os_error() == Some(109) => *eof = true,
        Err(error) => return Err(error),
    }
    Ok(())
}
#[cfg(test)]
#[path = "hosted_process_tests.rs"]
mod tests;
