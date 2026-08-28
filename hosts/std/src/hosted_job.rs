//! Std realization of one separately admitted executable resource.

use conduit_core::{
    ResourceDereferenceRequirement, ResourceReferenceAccessRefusal, ResourceReferenceBinding,
};
use conduit_semantic_catalog::{
    JobExitDisposition, JobLifecycleEvent, JobOutput, JobRequest, JobRequestRefusal,
    JobResourceUsage, JobStreamPressure, JobTerminalOutcome, JOB_EXECUTABLE_ACCESS_CLASS,
    JOB_EXECUTABLE_AUTHORITY, JOB_EXECUTABLE_CONTENT_PROFILE,
};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct AdmittedExecutable {
    pub binding: ResourceReferenceBinding,
    pub program: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct JobCancellation {
    cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedJobReport {
    pub lifecycle: Vec<JobLifecycleEvent>,
    pub stdout: JobOutput,
    pub stderr: JobOutput,
    pub usage: JobResourceUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedJobRefusal {
    InvalidRequest(JobRequestRefusal),
    Resource(ResourceReferenceAccessRefusal),
    ProgramNotAbsolute,
}

impl JobCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub fn run_bounded_job(
    request: &JobRequest,
    executable: &AdmittedExecutable,
    cancellation: &JobCancellation,
) -> Result<HostedJobReport, HostedJobRefusal> {
    request
        .validate()
        .map_err(HostedJobRefusal::InvalidRequest)?;
    if let Err(error) = admit_executable(request, executable) {
        if matches!(
            error,
            HostedJobRefusal::Resource(ResourceReferenceAccessRefusal::ResourceLost)
                | HostedJobRefusal::Resource(ResourceReferenceAccessRefusal::ResourceStale)
        ) {
            return Ok(empty_terminal_report(
                Vec::new(),
                request,
                Instant::now(),
                JobTerminalOutcome::ProviderLost {
                    message: "executable provider is unavailable".to_string(),
                },
            ));
        }
        return Err(error);
    }
    if !executable.program.is_absolute() {
        return Err(HostedJobRefusal::ProgramNotAbsolute);
    }

    let started = Instant::now();
    let mut lifecycle = vec![JobLifecycleEvent::Started];
    if cancellation.is_cancelled() {
        return Ok(empty_terminal_report(
            lifecycle,
            request,
            started,
            JobTerminalOutcome::Cancelled {
                message: "cancelled before launch".to_string(),
            },
        ));
    }

    let mut command = Command::new(&executable.program);
    command
        .args(&request.arguments)
        .env_clear()
        .envs(
            request
                .environment
                .iter()
                .map(|entry| (&entry.name, &entry.value)),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(empty_terminal_report(
                lifecycle,
                request,
                started,
                JobTerminalOutcome::Failed {
                    disposition: JobExitDisposition::Signal,
                    message: format!("launch refused: {error}"),
                },
            ))
        }
    };
    lifecycle.push(JobLifecycleEvent::Running);

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_limit = request.maximum_stdout_bytes as usize;
    let stderr_limit = request.maximum_stderr_bytes as usize;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, stderr_limit));

    let timeout = Duration::from_millis(request.timeout_millis);
    let terminal = loop {
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            break JobTerminalOutcome::Cancelled {
                message: "cancelled by admitted caller".to_string(),
            };
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break JobTerminalOutcome::TimedOut {
                timeout_millis: request.timeout_millis,
                message: "bounded execution deadline elapsed".to_string(),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                break JobTerminalOutcome::Completed {
                    disposition: exit_disposition(status),
                }
            }
            Ok(Some(status)) => {
                break JobTerminalOutcome::Failed {
                    disposition: exit_disposition(status),
                    message: "process returned a non-success disposition".to_string(),
                }
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break JobTerminalOutcome::ProviderLost {
                    message: format!("process provider lost: {error}"),
                };
            }
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    let usage = JobResourceUsage {
        elapsed_millis: elapsed_millis(started),
        stdout_observed_bytes: stdout.observed_bytes,
        stderr_observed_bytes: stderr.observed_bytes,
    };
    lifecycle.push(JobLifecycleEvent::Terminal(terminal));
    Ok(HostedJobReport {
        lifecycle,
        stdout: stdout.output(request.stdout_profile),
        stderr: stderr.output(request.stderr_profile),
        usage,
    })
}

fn admit_executable(
    request: &JobRequest,
    executable: &AdmittedExecutable,
) -> Result<(), HostedJobRefusal> {
    ResourceDereferenceRequirement {
        content_profile: conduit_core::kind_id(JOB_EXECUTABLE_CONTENT_PROFILE),
        access_class: conduit_core::ResourceClassId::from(JOB_EXECUTABLE_ACCESS_CLASS),
        authority_contract: conduit_core::AuthorityContractId::from(JOB_EXECUTABLE_AUTHORITY),
        maximum_bytes: request.executable.extent.bytes,
        maximum_items: request.executable.extent.items,
    }
    .admit(&request.executable, &executable.binding)
    .map(|_| ())
    .map_err(HostedJobRefusal::Resource)
}

fn empty_terminal_report(
    mut lifecycle: Vec<JobLifecycleEvent>,
    request: &JobRequest,
    started: Instant,
    terminal: JobTerminalOutcome,
) -> HostedJobReport {
    lifecycle.push(JobLifecycleEvent::Terminal(terminal));
    HostedJobReport {
        lifecycle,
        stdout: empty_output(request.stdout_profile),
        stderr: empty_output(request.stderr_profile),
        usage: JobResourceUsage {
            elapsed_millis: elapsed_millis(started),
            stdout_observed_bytes: 0,
            stderr_observed_bytes: 0,
        },
    }
}

fn empty_output(profile: conduit_semantic_catalog::JobOutputProfile) -> JobOutput {
    JobOutput {
        profile,
        bytes: Vec::new(),
        pressure: JobStreamPressure::WithinLimit,
        complete_artifact: None,
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(unix)]
fn exit_disposition(status: std::process::ExitStatus) -> JobExitDisposition {
    status
        .code()
        .map(JobExitDisposition::ExitCode)
        .unwrap_or(JobExitDisposition::Signal)
}

#[cfg(not(unix))]
fn exit_disposition(status: std::process::ExitStatus) -> JobExitDisposition {
    status
        .code()
        .map(JobExitDisposition::ExitCode)
        .unwrap_or(JobExitDisposition::Signal)
}

#[derive(Debug, Default)]
struct DrainedOutput {
    retained: Vec<u8>,
    observed_bytes: u64,
}

impl DrainedOutput {
    fn output(self, profile: conduit_semantic_catalog::JobOutputProfile) -> JobOutput {
        let pressure = if self.observed_bytes > self.retained.len() as u64 {
            JobStreamPressure::Truncated {
                observed_minimum_bytes: self.observed_bytes,
            }
        } else {
            JobStreamPressure::WithinLimit
        };
        JobOutput {
            profile,
            bytes: self.retained,
            pressure,
            complete_artifact: None,
        }
    }
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> DrainedOutput {
    let mut result = DrainedOutput::default();
    let mut buffer = [0_u8; 4_096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                result.observed_bytes = result.observed_bytes.saturating_add(read as u64);
                let remaining = limit.saturating_sub(result.retained.len());
                result
                    .retained
                    .extend_from_slice(&buffer[..read.min(remaining)]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    result
}

pub fn executable_path_is_explicit(path: &Path) -> bool {
    path.is_absolute()
}
