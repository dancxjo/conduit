//! Std realization of one separately admitted executable resource.

use conduit_core::{
    BootId, CapabilityId, HostId, OfferGeneration, PlannedGear, ResourceDereferenceRequirement,
    ResourcePoolId, ResourceReferenceAccessRefusal, ResourceReferenceBinding,
    ResourceSemanticIdentity,
};
use conduit_plot::rust_binding::BoundedBytes;
use conduit_semantic_catalog::{
    JobExitDisposition, JobLifecycleEvent, JobOutput, JobOutputBytes, JobRequest,
    JobRequestRefusal, JobResourceUsage, JobStreamPressure, JobTerminalOutcome, JobText,
};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TrustedExecutable {
    pool_id: ResourcePoolId,
    binding: ResourceReferenceBinding,
    program: PathBuf,
}

/// Current host-owned executable state. Portable callers can name a resource,
/// but only this trusted registry can associate its admitted handle with an OS
/// path and the exact provider generation selected by a Plan.
#[derive(Debug)]
pub struct TrustedJobProvider {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    capability_id: CapabilityId,
    executables: BTreeMap<ResourceSemanticIdentity, TrustedExecutable>,
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
    WrongPlannedKind,
    StalePlannedProvider,
    WrongPlannedCapability,
    WrongPlannedResource,
    WrongPlannedAuthority,
    ExecutableUnavailable,
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

impl TrustedJobProvider {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        capability_id: CapabilityId,
    ) -> Self {
        Self {
            host_id,
            boot_id,
            offer_generation,
            capability_id,
            executables: BTreeMap::new(),
        }
    }

    /// Installs trusted provider-local realization data. The path is never
    /// portable identity and never enters a Plot or Plan.
    pub fn register_executable(
        &mut self,
        pool_id: ResourcePoolId,
        binding: ResourceReferenceBinding,
        program: PathBuf,
    ) -> Result<(), HostedJobRefusal> {
        if !program.is_absolute() {
            return Err(HostedJobRefusal::ProgramNotAbsolute);
        }
        self.executables.insert(
            binding.identity,
            TrustedExecutable {
                pool_id,
                binding,
                program,
            },
        );
        Ok(())
    }

    pub fn run_bounded_job(
        &self,
        placement: &PlannedGear,
        request: &JobRequest,
        cancellation: &JobCancellation,
    ) -> Result<HostedJobReport, HostedJobRefusal> {
        let executable = match self.admit_planned_executable(placement, request) {
            Ok(executable) => executable,
            Err(HostedJobRefusal::Resource(
                ResourceReferenceAccessRefusal::ResourceLost
                | ResourceReferenceAccessRefusal::ResourceStale,
            )) => {
                return Ok(empty_terminal_report(
                    Vec::new(),
                    request,
                    Instant::now(),
                    JobTerminalOutcome::provider_lost(job_text(
                        "executable provider is unavailable".to_string(),
                    ))
                    .expect("bounded provider-loss outcome"),
                ));
            }
            Err(error) => return Err(error),
        };
        run_admitted_bounded_job(request, executable, cancellation)
    }

    fn admit_planned_executable<'a>(
        &'a self,
        placement: &PlannedGear,
        request: &JobRequest,
    ) -> Result<&'a TrustedExecutable, HostedJobRefusal> {
        request
            .validate_job()
            .map_err(HostedJobRefusal::InvalidRequest)?;
        if placement.kind_id.as_str() != conduit_semantic_catalog::JOB_RUN_KIND {
            return Err(HostedJobRefusal::WrongPlannedKind);
        }
        if placement.host_id != self.host_id
            || placement.boot_id != self.boot_id
            || placement.offer_generation != self.offer_generation
        {
            return Err(HostedJobRefusal::StalePlannedProvider);
        }
        if placement.capability_id != self.capability_id {
            return Err(HostedJobRefusal::WrongPlannedCapability);
        }
        let [resource] = placement.resources.as_slice() else {
            return Err(HostedJobRefusal::WrongPlannedResource);
        };
        if resource.class_id != request.executable().get().access_class {
            return Err(HostedJobRefusal::WrongPlannedResource);
        }
        let Some(content) = &resource.content else {
            return Err(HostedJobRefusal::WrongPlannedResource);
        };
        if content
            .contract
            .accepts_reference(request.executable().get())
            .is_err()
        {
            return Err(HostedJobRefusal::WrongPlannedResource);
        }
        let [authority] = placement.authority.as_slice() else {
            return Err(HostedJobRefusal::WrongPlannedAuthority);
        };
        let authority_is_selected_for_this_operation = authority.host_id == placement.host_id
            && authority.boot_id == placement.boot_id
            && authority.capability_id == placement.capability_id
            && authority.subject_kind == placement.kind_id
            && placement
                .host_calls
                .iter()
                .any(|call| call.contract_id == authority.host_call_contract_id);
        if !authority_is_selected_for_this_operation {
            return Err(HostedJobRefusal::WrongPlannedAuthority);
        }
        let executable = self
            .executables
            .get(&request.executable().get().identity)
            .ok_or(HostedJobRefusal::ExecutableUnavailable)?;
        if executable.pool_id != resource.pool_id
            || executable.binding.authority_contract != authority.contract_id
            || executable.binding.authority_grant != authority.grant_id
        {
            return Err(HostedJobRefusal::WrongPlannedAuthority);
        }
        ResourceDereferenceRequirement {
            content_profile: request.executable().get().content_profile.clone(),
            access_class: resource.class_id.clone(),
            authority_contract: authority.contract_id.clone(),
            maximum_bytes: request.executable().get().extent.bytes,
            maximum_items: request.executable().get().extent.items,
        }
        .admit(request.executable().get(), &executable.binding)
        .map_err(HostedJobRefusal::Resource)?;
        Ok(executable)
    }
}

fn run_admitted_bounded_job(
    request: &JobRequest,
    executable: &TrustedExecutable,
    cancellation: &JobCancellation,
) -> Result<HostedJobReport, HostedJobRefusal> {
    let started = Instant::now();
    let mut lifecycle = vec![JobLifecycleEvent::Started];
    if cancellation.is_cancelled() {
        return Ok(empty_terminal_report(
            lifecycle,
            request,
            started,
            JobTerminalOutcome::cancelled(job_text("cancelled before launch".to_string()))
                .expect("bounded cancellation outcome"),
        ));
    }

    let mut command = Command::new(&executable.program);
    command
        .args(
            request
                .arguments()
                .get()
                .iter()
                .map(|argument| argument.get()),
        )
        .env_clear()
        .envs(
            request
                .environment()
                .get()
                .iter()
                .map(|entry| (entry.name().get(), entry.value().get())),
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
                JobTerminalOutcome::failed(
                    JobExitDisposition::Signal,
                    job_text(format!("launch refused: {error}")),
                )
                .expect("bounded launch-refusal outcome"),
            ))
        }
    };
    lifecycle.push(JobLifecycleEvent::Running);

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_limit = *request.maximum_stdout_bytes() as usize;
    let stderr_limit = *request.maximum_stderr_bytes() as usize;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, stderr_limit));

    let timeout = Duration::from_millis(*request.timeout_millis());
    let terminal = loop {
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            break JobTerminalOutcome::cancelled(job_text(
                "cancelled by admitted caller".to_string(),
            ))
            .expect("bounded cancellation outcome");
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break JobTerminalOutcome::timed_out(
                job_text("bounded execution deadline elapsed".to_string()),
                *request.timeout_millis(),
            )
            .expect("bounded timeout outcome");
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                break JobTerminalOutcome::completed(exit_disposition(status))
                    .expect("completed outcome")
            }
            Ok(Some(status)) => {
                break JobTerminalOutcome::failed(
                    exit_disposition(status),
                    job_text("process returned a non-success disposition".to_string()),
                )
                .expect("bounded failure outcome")
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break JobTerminalOutcome::provider_lost(job_text(format!(
                    "process provider lost: {error}"
                )))
                .expect("bounded provider-loss outcome");
            }
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    let usage = JobResourceUsage::new(
        elapsed_millis(started),
        stderr.observed_bytes,
        stdout.observed_bytes,
    )
    .expect("bounded Job resource usage");
    lifecycle.push(JobLifecycleEvent::Terminal(terminal));
    Ok(HostedJobReport {
        lifecycle,
        stdout: stdout.output(*request.stdout_profile()),
        stderr: stderr.output(*request.stderr_profile()),
        usage,
    })
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
        stdout: empty_output(*request.stdout_profile()),
        stderr: empty_output(*request.stderr_profile()),
        usage: JobResourceUsage::new(elapsed_millis(started), 0, 0)
            .expect("bounded empty Job resource usage"),
    }
}

fn empty_output(profile: conduit_semantic_catalog::JobOutputProfile) -> JobOutput {
    JobOutput::new(
        output_bytes(Vec::new()),
        None,
        JobStreamPressure::WithinLimit,
        profile,
    )
    .expect("bounded empty Job output")
}

fn job_text(mut value: String) -> JobText {
    if value.len() > JobText::MAXIMUM_BYTES {
        let mut boundary = JobText::MAXIMUM_BYTES;
        while !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        value.truncate(boundary);
    }
    JobText::new(value).expect("bounded Job text")
}

fn output_bytes(value: Vec<u8>) -> JobOutputBytes {
    let value = BoundedBytes::<65536>::new(&value).expect("admitted Job output bound");
    JobOutputBytes::new(value).expect("bounded Job output")
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
            JobStreamPressure::truncated(self.observed_bytes).expect("bounded stream pressure")
        } else {
            JobStreamPressure::WithinLimit
        };
        JobOutput::new(output_bytes(self.retained), None, pressure, profile)
            .expect("bounded Job output")
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
