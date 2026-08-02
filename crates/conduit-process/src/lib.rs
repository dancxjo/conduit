//! Closed-inventory hosted provider for `conduit.host/process/exec`.
//!
//! This example Linux-class provider launches the current Conduct artifact at
//! a private fixture entrypoint. It never invokes a shell or searches `PATH`;
//! the environment is cleared and authored argv remains literal.

use std::env;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};

use conduit_core::SemanticHash;
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};
use conduit_std::{
    PROCESS_MAX_ARGUMENTS, PROCESS_MAX_CHUNK_BYTES, PROCESS_MAX_ENVIRONMENT,
    PROCESS_MAX_ENVIRONMENT_NAME_BYTES, PROCESS_MAX_ENVIRONMENT_VALUE_BYTES,
    PROCESS_MAX_EVIDENCE_EVENTS, PROCESS_MAX_STREAM_BYTES,
};

pub const EXAMPLE_EXECUTABLE_RESOURCE: &str = "conduit.executable/process-fixture";
pub const EXAMPLE_WORKING_RESOURCE: &str = "conduit.resource/process-working-root";
pub const EXAMPLE_GRANT: &str = "conduit.grant/process-exec";
pub const FIXTURE_ENTRYPOINT: &str = "__conduit_process_fixture__";
/// Hosted adapters may need more fixed arguments than the portable authored
/// exec contract while remaining explicitly bounded.
pub const SUPERVISED_PROCESS_MAX_ARGUMENTS: usize = 64;

const CONTRACT_ID: &str = "conduit.host/process/exec";
const IMPLEMENTATION_ID: &str = "conduit/process-exec-hosted";
const ARTIFACT_ID: &str = "conduit/process-exec-hosted-artifact";
const MAXIMUM_DEADLINE_MILLIS: u64 = 10_000;
const EXPECTED_KEYS: &[&str] = &[
    "program",
    "argv",
    "environment",
    "working_resource",
    "grant",
    "stdin_close",
    "maximum_stdin_bytes",
    "maximum_stdout_bytes",
    "maximum_stderr_bytes",
    "maximum_chunk_bytes",
    "maximum_pending_operations",
    "maximum_processes",
    "maximum_child_processes",
    "maximum_descriptors",
    "maximum_environment_bytes",
    "maximum_work",
    "maximum_evidence_events",
    "deadline_ticks",
    "graceful_signal",
    "graceful_ticks",
    "forced_ticks",
    "cancellation",
];

fn contract() -> &'static conduit_core::NodeContract<'static> {
    conduit_std::standard_node_contract(CONTRACT_ID).expect("process exec contract is published")
}

fn resolution_error(code: &'static str, node: &Node, detail: &str) -> ResolutionError {
    ResolutionError::new(code, format!("process exec `{}` {detail}", node.id))
}

fn runtime_error(code: &'static str, detail: impl Into<String>) -> RuntimeError {
    RuntimeError::new(code, detail)
}

fn exact_reference(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::Reference(value) | SourceValue::ContractReference(value))
            if value == expected
    )
}

fn exact_secret(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    )
}

fn required_usize(node: &Node, key: &str) -> Result<usize, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => usize::try_from(*value)
            .map_err(|_| resolution_error("CND-EXEC-005", node, "has an invalid finite bound")),
        _ => Err(resolution_error(
            "CND-EXEC-005",
            node,
            "is missing a finite bound",
        )),
    }
}

fn required_u64(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value)
            .map_err(|_| resolution_error("CND-EXEC-005", node, "has an invalid finite bound")),
        _ => Err(resolution_error(
            "CND-EXEC-005",
            node,
            "is missing a finite bound",
        )),
    }
}

fn record_text<'a>(
    node: &'a Node,
    key: &str,
    maximum_fields: usize,
    maximum_name_bytes: usize,
    maximum_value_bytes: usize,
) -> Result<Vec<(&'a str, &'a str)>, ResolutionError> {
    let Some(SourceValue::Record(fields)) = node.config_value(key) else {
        return Err(resolution_error(
            "CND-EXEC-003",
            node,
            "has a non-record command field",
        ));
    };
    if fields.len() > maximum_fields {
        return Err(resolution_error(
            "CND-EXEC-003",
            node,
            "exceeds a command record bound",
        ));
    }
    fields
        .iter()
        .map(|(name, value)| {
            let SourceValue::Text(value) = value else {
                return Err(resolution_error(
                    "CND-EXEC-003",
                    node,
                    "contains a non-text command record value",
                ));
            };
            if name.is_empty()
                || name.len() > maximum_name_bytes
                || value.len() > maximum_value_bytes
                || name.as_bytes().contains(&0)
                || value.as_bytes().contains(&0)
            {
                return Err(resolution_error(
                    "CND-EXEC-003",
                    node,
                    "exceeds a command record byte bound",
                ));
            }
            Ok((name.as_str(), value.as_str()))
        })
        .collect()
}

fn argv(node: &Node) -> Result<Vec<&str>, ResolutionError> {
    let mut fields = record_text(
        node,
        "argv",
        PROCESS_MAX_ARGUMENTS,
        8,
        conduit_std::PROCESS_MAX_ARGUMENT_BYTES,
    )?;
    fields.sort_by_key(|(name, _)| *name);
    for (index, (name, _)) in fields.iter().enumerate() {
        if *name != format!("arg{index}") {
            return Err(resolution_error(
                "CND-EXEC-002",
                node,
                "argv keys must be contiguous arg0 through argN",
            ));
        }
    }
    Ok(fields.into_iter().map(|(_, value)| value).collect())
}

fn environment(node: &Node) -> Result<Vec<(&str, &str)>, ResolutionError> {
    let fields = record_text(
        node,
        "environment",
        PROCESS_MAX_ENVIRONMENT,
        PROCESS_MAX_ENVIRONMENT_NAME_BYTES,
        PROCESS_MAX_ENVIRONMENT_VALUE_BYTES,
    )?;
    if fields
        .iter()
        .any(|(name, _)| name.as_bytes().contains(&b'='))
    {
        return Err(resolution_error(
            "CND-EXEC-004",
            node,
            "contains an invalid environment name",
        ));
    }
    Ok(fields)
}

fn validate_config(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() != EXPECTED_KEYS.len()
        || EXPECTED_KEYS
            .iter()
            .any(|key| !node.config.iter().any(|entry| entry.key == *key))
    {
        return Err(resolution_error(
            "CND-EXEC-010",
            node,
            "does not match the exact current config",
        ));
    }
    if !exact_reference(node, "program", EXAMPLE_EXECUTABLE_RESOURCE)
        || !exact_secret(node, "working_resource", EXAMPLE_WORKING_RESOURCE)
        || !exact_secret(node, "grant", EXAMPLE_GRANT)
        || node.config("stdin_close") != Some("close-after-input")
        || !matches!(
            node.config("cancellation"),
            Some("none" | "cancel-before-spawn" | "cancel-after-spawn")
        )
    {
        return Err(resolution_error(
            "CND-EXEC-010",
            node,
            "names unsupported provider facts",
        ));
    }
    let _arguments = argv(node)?;
    let environment = environment(node)?;
    let environment_bytes = environment
        .iter()
        .map(|(name, value)| name.len() + value.len())
        .sum::<usize>();
    let stdin = required_usize(node, "maximum_stdin_bytes")?;
    let stdout = required_usize(node, "maximum_stdout_bytes")?;
    let stderr = required_usize(node, "maximum_stderr_bytes")?;
    let chunk = required_usize(node, "maximum_chunk_bytes")?;
    let pending = required_usize(node, "maximum_pending_operations")?;
    let processes = required_usize(node, "maximum_processes")?;
    let children = required_usize(node, "maximum_child_processes")?;
    let descriptors = required_usize(node, "maximum_descriptors")?;
    let environment_limit = required_usize(node, "maximum_environment_bytes")?;
    let work = required_usize(node, "maximum_work")?;
    let evidence = required_usize(node, "maximum_evidence_events")?;
    let deadline = required_u64(node, "deadline_ticks")?;
    let graceful_signal = required_u64(node, "graceful_signal")?;
    let graceful_ticks = required_u64(node, "graceful_ticks")?;
    let forced_ticks = required_u64(node, "forced_ticks")?;
    if stdin == 0
        || stdin > PROCESS_MAX_STREAM_BYTES
        || stdout == 0
        || stdout > PROCESS_MAX_STREAM_BYTES
        || stderr == 0
        || stderr > PROCESS_MAX_STREAM_BYTES
        || chunk == 0
        || chunk > PROCESS_MAX_CHUNK_BYTES
        || pending < 3
        || processes != 1
        || children != 0
        || descriptors < 3
        || environment_bytes > environment_limit
        || work == 0
        || evidence == 0
        || evidence > PROCESS_MAX_EVIDENCE_EVENTS
        || deadline == 0
        || deadline > MAXIMUM_DEADLINE_MILLIS
        || graceful_signal != 9
        || graceful_ticks == 0
        || forced_ticks == 0
    {
        return Err(resolution_error(
            "CND-EXEC-005",
            node,
            "exceeds the installed provider profile",
        ));
    }
    Ok(())
}

fn read_bounded(mut reader: impl Read, maximum: usize) -> io::Result<Vec<u8>> {
    let take = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(maximum.min(PROCESS_MAX_CHUNK_BYTES));
    reader.by_ref().take(take).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn collect_stdin_chunks<'a>(
    chunks: impl IntoIterator<Item = &'a [u8]>,
    maximum_chunk: usize,
    maximum_total: usize,
) -> Result<Vec<u8>, RuntimeError> {
    let mut stdin = Vec::with_capacity(maximum_total.min(PROCESS_MAX_CHUNK_BYTES));
    for chunk in chunks {
        if chunk.len() > maximum_chunk {
            return Err(runtime_error(
                "CND-EXEC-006",
                "process stdin chunk exceeds its exact byte ceiling",
            ));
        }
        let next_len = stdin.len().checked_add(chunk.len()).ok_or_else(|| {
            runtime_error("CND-EXEC-006", "process stdin byte accounting overflowed")
        })?;
        if next_len > maximum_total {
            return Err(runtime_error(
                "CND-EXEC-006",
                "process stdin stream exceeds its exact byte ceiling",
            ));
        }
        stdin.extend_from_slice(chunk);
    }
    Ok(stdin)
}

/// Generic exact command admitted by a domain adapter.
///
/// The semantic node is intentionally absent: media, repository, compiler,
/// and other adapters translate their own contracts into this already-bound
/// host request. This layer never parses shell text or performs `PATH` lookup.
pub struct SupervisedProcessRequest<'a> {
    pub executable: &'a Path,
    pub argv: &'a [OsString],
    pub environment: &'a [(OsString, OsString)],
    pub working_directory: &'a Path,
    pub stdin: &'a [u8],
    pub limits: SupervisedProcessLimits,
    pub cancellation: SupervisedProcessCancellation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisedProcessLimits {
    pub maximum_arguments: usize,
    pub maximum_stdin_bytes: usize,
    pub maximum_stdout_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub maximum_processes: usize,
    pub maximum_child_processes: usize,
    pub maximum_threads: usize,
    pub maximum_descriptors: usize,
    pub deadline_millis: u64,
    pub cleanup_millis: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisedProcessCancellation {
    None,
    BeforeSpawn,
    AfterSpawn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisedProcessTerminal {
    Exited(i32),
    Signaled,
    Cancelled { forced: bool },
    DeadlineExceeded { forced: bool },
    ChildProcessLimitExceeded,
    ThreadLimitExceeded,
    DescriptorLimitExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisedProcessResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub terminal: SupervisedProcessTerminal,
    pub cleanup_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisedProcessError {
    InvalidRequest,
    InputOverflow,
    SpawnFailed,
    PipeUnavailable,
    InputWriteFailed,
    OutputReadFailed,
    OutputOverflow,
    StderrOverflow,
    WaitFailed,
    CleanupFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum ObservedLimit {
    ChildProcess,
    Thread,
    Descriptor,
}

#[cfg(target_os = "linux")]
fn observed_limit(pid: u32, limits: SupervisedProcessLimits) -> Option<ObservedLimit> {
    let task_root = format!("/proc/{pid}/task");
    let thread_count = std::fs::read_dir(&task_root)
        .ok()?
        .take(limits.maximum_threads + 1)
        .count();
    if thread_count > limits.maximum_threads {
        return Some(ObservedLimit::Thread);
    }
    let descriptor_count = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .ok()?
        .take(limits.maximum_descriptors + 1)
        .count();
    if descriptor_count > limits.maximum_descriptors {
        return Some(ObservedLimit::Descriptor);
    }
    if limits.maximum_child_processes == 0 {
        for task in std::fs::read_dir(task_root)
            .ok()?
            .take(limits.maximum_threads + 1)
        {
            let task = task.ok()?;
            let mut children = String::new();
            std::fs::File::open(task.path().join("children"))
                .ok()?
                .take(4097)
                .read_to_string(&mut children)
                .ok()?;
            if children.len() > 4096 {
                return Some(ObservedLimit::ChildProcess);
            }
            if !children.trim().is_empty() {
                return Some(ObservedLimit::ChildProcess);
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn observed_limit(_pid: u32, _limits: SupervisedProcessLimits) -> Option<ObservedLimit> {
    None
}

fn terminate_and_wait(child: &mut Child) -> Result<(), SupervisedProcessError> {
    #[cfg(unix)]
    {
        let pid = i32::try_from(child.id()).map_err(|_| SupervisedProcessError::CleanupFailed)?;
        killpg(Pid::from_raw(pid), Signal::SIGKILL)
            .map_err(|_| SupervisedProcessError::CleanupFailed)?;
    }
    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|_| SupervisedProcessError::CleanupFailed)?;
    child
        .wait()
        .map_err(|_| SupervisedProcessError::CleanupFailed)?;
    Ok(())
}

impl SupervisedProcessError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "CND-EXEC-005",
            Self::InputOverflow => "CND-EXEC-006",
            Self::SpawnFailed => "CND-EXEC-008",
            Self::PipeUnavailable
            | Self::InputWriteFailed
            | Self::OutputReadFailed
            | Self::WaitFailed
            | Self::CleanupFailed => "CND-EXEC-011",
            Self::OutputOverflow => "CND-EXEC-013",
            Self::StderrOverflow => "CND-EXEC-014",
        }
    }
}

/// Execute one exact argv vector through the shared bounded process boundary.
///
/// The caller must already have selected and authorized the exact executable.
/// This function clears ambient environment, never invokes a shell, retains
/// stdout/stderr separately, and always waits after forced termination.
pub fn run_supervised_process(
    request: &SupervisedProcessRequest<'_>,
) -> Result<SupervisedProcessResult, SupervisedProcessError> {
    let limits = request.limits;
    if !request.executable.is_absolute()
        || limits.maximum_arguments == 0
        || limits.maximum_arguments > SUPERVISED_PROCESS_MAX_ARGUMENTS
        || request.argv.len() > limits.maximum_arguments
        || request.environment.len() > PROCESS_MAX_ENVIRONMENT
        || limits.maximum_stdin_bytes == 0
        || limits.maximum_stdout_bytes == 0
        || limits.maximum_stderr_bytes == 0
        || limits.maximum_processes != 1
        || limits.maximum_child_processes != 0
        || limits.maximum_threads == 0
        || limits.maximum_descriptors < 3
        || limits.deadline_millis == 0
        || limits.cleanup_millis == 0
    {
        return Err(SupervisedProcessError::InvalidRequest);
    }
    if request.stdin.len() > limits.maximum_stdin_bytes {
        return Err(SupervisedProcessError::InputOverflow);
    }
    if request.cancellation == SupervisedProcessCancellation::BeforeSpawn {
        return Ok(SupervisedProcessResult {
            stdout: Vec::new(),
            stderr: Vec::new(),
            terminal: SupervisedProcessTerminal::Cancelled { forced: false },
            cleanup_complete: true,
        });
    }
    let mut command = Command::new(request.executable);
    command
        .args(request.argv)
        .env_clear()
        .envs(request.environment.iter().cloned())
        .current_dir(request.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| SupervisedProcessError::SpawnFailed)?;
    if request.cancellation == SupervisedProcessCancellation::AfterSpawn {
        terminate_and_wait(&mut child)?;
        return Ok(SupervisedProcessResult {
            stdout: Vec::new(),
            stderr: Vec::new(),
            terminal: SupervisedProcessTerminal::Cancelled { forced: true },
            cleanup_complete: true,
        });
    }
    let child_stdin = child
        .stdin
        .take()
        .ok_or(SupervisedProcessError::PipeUnavailable)?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or(SupervisedProcessError::PipeUnavailable)?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or(SupervisedProcessError::PipeUnavailable)?;
    let stdin = request.stdin;
    let started = Instant::now();
    let (stdout, stderr, terminal) = thread::scope(|scope| {
        let writer = scope.spawn(move || {
            let mut child_stdin = child_stdin;
            child_stdin.write_all(stdin)?;
            child_stdin.flush()
        });
        let stdout_reader =
            scope.spawn(move || read_bounded(child_stdout, limits.maximum_stdout_bytes));
        let stderr_reader =
            scope.spawn(move || read_bounded(child_stderr, limits.maximum_stderr_bytes));
        let terminal = loop {
            if let Some(limit) = observed_limit(child.id(), limits) {
                terminate_and_wait(&mut child)?;
                break match limit {
                    ObservedLimit::ChildProcess => {
                        SupervisedProcessTerminal::ChildProcessLimitExceeded
                    }
                    ObservedLimit::Thread => SupervisedProcessTerminal::ThreadLimitExceeded,
                    ObservedLimit::Descriptor => SupervisedProcessTerminal::DescriptorLimitExceeded,
                };
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if let Some(code) = status.code() {
                        SupervisedProcessTerminal::Exited(code)
                    } else {
                        SupervisedProcessTerminal::Signaled
                    };
                }
                Ok(None) if started.elapsed() < Duration::from_millis(limits.deadline_millis) => {
                    thread::sleep(Duration::from_millis(1));
                }
                Ok(None) => {
                    terminate_and_wait(&mut child)?;
                    break SupervisedProcessTerminal::DeadlineExceeded { forced: true };
                }
                Err(_) => return Err(SupervisedProcessError::WaitFailed),
            }
        };
        writer
            .join()
            .map_err(|_| SupervisedProcessError::InputWriteFailed)?
            .map_err(|_| SupervisedProcessError::InputWriteFailed)?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| SupervisedProcessError::OutputReadFailed)?
            .map_err(|_| SupervisedProcessError::OutputReadFailed)?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| SupervisedProcessError::OutputReadFailed)?
            .map_err(|_| SupervisedProcessError::OutputReadFailed)?;
        Ok::<_, SupervisedProcessError>((stdout, stderr, terminal))
    })?;
    if stdout.len() > limits.maximum_stdout_bytes {
        return Err(SupervisedProcessError::OutputOverflow);
    }
    if stderr.len() > limits.maximum_stderr_bytes {
        return Err(SupervisedProcessError::StderrOverflow);
    }
    Ok(SupervisedProcessResult {
        stdout,
        stderr,
        terminal,
        cleanup_complete: true,
    })
}

struct ProcessHandler;

impl Handler for ProcessHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_config(node).map_err(|error| runtime_error(error.code, error.message))?;
        let contract = contract();
        let maximum_stdin = required_usize(node, "maximum_stdin_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let maximum_chunk = required_usize(node, "maximum_chunk_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let stdin = collect_stdin_chunks(
            inputs
                .iter()
                .filter(|input| input.value_type == contract.inputs[0].value_type)
                .map(|input| input.bytes.as_slice()),
            maximum_chunk,
            maximum_stdin,
        )?;
        if node.config("cancellation") == Some("cancel-before-spawn") {
            return Err(runtime_error(
                "CND-EXEC-009",
                "process cancellation completed before spawn",
            ));
        }
        let arguments = argv(node).map_err(|error| runtime_error(error.code, error.message))?;
        let environment =
            environment(node).map_err(|error| runtime_error(error.code, error.message))?;
        let executable = env::current_exe().map_err(|error| {
            runtime_error(
                "CND-EXEC-008",
                format!("exact current executable is unavailable: {error}"),
            )
        })?;
        let maximum_stdout = required_usize(node, "maximum_stdout_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let maximum_stderr = required_usize(node, "maximum_stderr_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let deadline_millis = required_u64(node, "deadline_ticks")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let mut supervised_argv = Vec::with_capacity(arguments.len() + 1);
        supervised_argv.push(OsString::from(FIXTURE_ENTRYPOINT));
        supervised_argv.extend(arguments.into_iter().map(OsString::from));
        let environment = environment
            .into_iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect::<Vec<_>>();
        let cancellation = match node.config("cancellation") {
            Some("cancel-after-spawn") => SupervisedProcessCancellation::AfterSpawn,
            _ => SupervisedProcessCancellation::None,
        };
        let result = run_supervised_process(&SupervisedProcessRequest {
            executable: &executable,
            argv: &supervised_argv,
            environment: &environment,
            working_directory: Path::new("/"),
            stdin: &stdin,
            limits: SupervisedProcessLimits {
                maximum_arguments: PROCESS_MAX_ARGUMENTS + 1,
                maximum_stdin_bytes: maximum_stdin,
                maximum_stdout_bytes: maximum_stdout,
                maximum_stderr_bytes: maximum_stderr,
                maximum_processes: 1,
                maximum_child_processes: 0,
                maximum_threads: 1,
                maximum_descriptors: required_usize(node, "maximum_descriptors")
                    .map_err(|error| runtime_error(error.code, error.message))?,
                deadline_millis,
                cleanup_millis: required_u64(node, "forced_ticks")
                    .map_err(|error| runtime_error(error.code, error.message))?,
            },
            cancellation,
        })
        .map_err(|error| runtime_error(error.code(), format!("process boundary: {error:?}")))?;
        match result.terminal {
            SupervisedProcessTerminal::Exited(0) => {}
            SupervisedProcessTerminal::Cancelled { .. } => {
                return Err(runtime_error(
                    "CND-EXEC-009",
                    "process cancellation completed with bounded cleanup",
                ));
            }
            SupervisedProcessTerminal::DeadlineExceeded { .. } => {
                return Err(runtime_error(
                    "CND-EXEC-012",
                    "process deadline expired; forced termination and wait completed",
                ));
            }
            SupervisedProcessTerminal::ChildProcessLimitExceeded
            | SupervisedProcessTerminal::ThreadLimitExceeded
            | SupervisedProcessTerminal::DescriptorLimitExceeded => {
                return Err(runtime_error(
                    "CND-EXEC-005",
                    "process crossed an observed runtime resource ceiling",
                ));
            }
            SupervisedProcessTerminal::Exited(_) | SupervisedProcessTerminal::Signaled => {
                return Err(runtime_error(
                    "CND-EXEC-015",
                    "process exited unsuccessfully",
                ));
            }
        }
        Ok(vec![
            Value {
                value_type: contract.outputs[0].value_type,
                bytes: result.stdout,
            },
            Value {
                value_type: contract.outputs[1].value_type,
                bytes: result.stderr,
            },
        ])
    }
}

/// Explicitly installs the bounded closed-inventory process provider.
pub fn register_hosted_process_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    static AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x50; 32])];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: contract(),
        implementation_id: IMPLEMENTATION_ID,
        artifact_id: ARTIFACT_ID,
        entrypoint: "process-exec",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &AUTHORITY,
        factory: || Box::new(ProcessHandler),
        validate_config,
    })
}

/// Redacted facts suitable for host reports and Patchbay.
#[must_use]
pub fn provider_description() -> Vec<(&'static str, String)> {
    vec![
        ("program_inventory", "closed".to_owned()),
        ("shell", "unsupported".to_owned()),
        ("path_search", "disabled".to_owned()),
        ("ambient_environment", "cleared".to_owned()),
        ("maximum_arguments", PROCESS_MAX_ARGUMENTS.to_string()),
        (
            "maximum_environment_additions",
            PROCESS_MAX_ENVIRONMENT.to_string(),
        ),
        ("maximum_stream_bytes", PROCESS_MAX_STREAM_BYTES.to_string()),
        ("maximum_chunk_bytes", PROCESS_MAX_CHUNK_BYTES.to_string()),
        ("maximum_processes", "1".to_owned()),
        ("maximum_child_processes", "0".to_owned()),
        ("time_basis", "monotonic-millisecond".to_owned()),
        ("working_resource", "protected".to_owned()),
        ("grant", "protected".to_owned()),
    ]
}

/// Run the private child entrypoint when the current executable was launched
/// by this provider. Returns `None` for every ordinary CLI invocation.
#[must_use]
pub fn fixture_entrypoint() -> Option<ExitCode> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(OsString::from(FIXTURE_ENTRYPOINT).as_os_str()) {
        return None;
    }
    let first_argument = arguments.next().unwrap_or_default();
    let mut stdin = Vec::with_capacity(PROCESS_MAX_CHUNK_BYTES);
    if io::stdin()
        .take((PROCESS_MAX_STREAM_BYTES as u64).saturating_add(1))
        .read_to_end(&mut stdin)
        .is_err()
        || stdin.len() > PROCESS_MAX_STREAM_BYTES
    {
        return Some(ExitCode::from(125));
    }
    let result = match first_argument.to_str() {
        Some("--independent-streams") => io::stdout()
            .write_all(&stdin)
            .and_then(|()| io::stderr().write_all(b"diagnostic\n"))
            .map(|()| 0),
        Some("--exit-7") => Ok(7),
        Some("--abort") => std::process::abort(),
        Some("--sleep") => {
            thread::sleep(Duration::from_secs(30));
            Ok(0)
        }
        _ => io::stdout().write_all(&stdin).map(|()| 0),
    };
    Some(ExitCode::from(result.unwrap_or(125)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn supervised(
        executable: &str,
        arguments: &[&str],
        stdout: usize,
        stderr: usize,
        deadline_millis: u64,
    ) -> Result<SupervisedProcessResult, SupervisedProcessError> {
        let argv = arguments.iter().map(OsString::from).collect::<Vec<_>>();
        run_supervised_process(&SupervisedProcessRequest {
            executable: Path::new(executable),
            argv: &argv,
            environment: &[],
            working_directory: Path::new("/"),
            stdin: &[],
            limits: SupervisedProcessLimits {
                maximum_arguments: 8,
                maximum_stdin_bytes: 1,
                maximum_stdout_bytes: stdout,
                maximum_stderr_bytes: stderr,
                maximum_processes: 1,
                maximum_child_processes: 0,
                maximum_threads: 1,
                maximum_descriptors: 16,
                deadline_millis,
                cleanup_millis: 100,
            },
            cancellation: SupervisedProcessCancellation::None,
        })
    }

    #[test]
    fn stdin_collection_accepts_empty_and_multiple_bounded_chunks() {
        assert_eq!(
            collect_stdin_chunks(std::iter::empty(), 4, 8).unwrap(),
            Vec::<u8>::new()
        );
        assert_eq!(
            collect_stdin_chunks([b"ab".as_slice(), b"cd".as_slice()], 2, 4).unwrap(),
            b"abcd"
        );
        assert_eq!(
            collect_stdin_chunks([b"abc".as_slice()], 2, 4)
                .expect_err("oversized chunk is rejected")
                .code,
            "CND-EXEC-006"
        );
        assert_eq!(
            collect_stdin_chunks([b"ab".as_slice(), b"cd".as_slice()], 2, 3)
                .expect_err("oversized stream is rejected")
                .code,
            "CND-EXEC-006"
        );
    }

    #[test]
    fn provider_description_redacts_resources_and_environment_values() {
        let description = provider_description();
        assert!(
            description
                .iter()
                .any(|(key, value)| { *key == "ambient_environment" && value == "cleared" })
        );
        let rendered = format!("{description:?}");
        assert!(!rendered.contains(EXAMPLE_GRANT));
        assert!(!rendered.contains(EXAMPLE_WORKING_RESOURCE));
    }

    #[test]
    fn checked_example_resolves_only_after_explicit_provider_installation() {
        let panel =
            conduit_panel::parse(include_str!("../../../examples/process-exec.panel")).unwrap();
        let registry = Registry::hosted_primitives();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("process provider is opt-in")
                .code,
            "CND-IMP-001"
        );
        let mut registry = Registry::hosted_primitives();
        register_hosted_process_provider(&mut registry).unwrap();
        registry
            .resolve(&panel)
            .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message));
    }

    #[cfg(unix)]
    #[test]
    fn exact_argv_has_no_shell_interpretation() {
        let injection = "$(printf injected);*;https://example.invalid";
        let result = supervised("/usr/bin/printf", &["%s", injection], 256, 64, 500).unwrap();
        assert_eq!(result.stdout, injection.as_bytes());
        assert_eq!(result.terminal, SupervisedProcessTerminal::Exited(0));
    }

    #[cfg(unix)]
    #[test]
    fn output_stderr_deadline_child_signal_and_partial_output_are_bounded() {
        assert_eq!(
            supervised("/usr/bin/printf", &["123456789"], 8, 64, 500),
            Err(SupervisedProcessError::OutputOverflow)
        );
        assert_eq!(
            supervised("/bin/sh", &["-c", "printf 123456789 >&2"], 64, 8, 500),
            Err(SupervisedProcessError::StderrOverflow)
        );

        let deadline = supervised("/usr/bin/sleep", &["30"], 64, 64, 5).unwrap();
        assert_eq!(
            deadline.terminal,
            SupervisedProcessTerminal::DeadlineExceeded { forced: true }
        );
        assert!(deadline.cleanup_complete);

        let child = supervised("/bin/sh", &["-c", "sleep 30 & wait"], 64, 64, 500).unwrap();
        assert_eq!(
            child.terminal,
            SupervisedProcessTerminal::ChildProcessLimitExceeded
        );
        assert!(child.cleanup_complete);

        let signaled = supervised("/bin/sh", &["-c", "kill -TERM $$"], 64, 64, 500).unwrap();
        assert_eq!(signaled.terminal, SupervisedProcessTerminal::Signaled);

        let partial =
            supervised("/bin/sh", &["-c", "printf partial; exit 7"], 64, 64, 500).unwrap();
        assert_eq!(partial.stdout, b"partial");
        assert_eq!(partial.terminal, SupervisedProcessTerminal::Exited(7));
    }
}
