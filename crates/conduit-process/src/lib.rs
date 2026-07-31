//! Closed-inventory hosted provider for `conduit.host/process/exec`.
//!
//! This example Linux-class provider launches the current Conduct artifact at
//! a private fixture entrypoint. It never invokes a shell or searches `PATH`;
//! the environment is cleared and authored argv remains literal.

use std::env;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        let input = inputs
            .first()
            .filter(|input| input.value_type == contract.inputs[0].value_type)
            .ok_or_else(|| runtime_error("CND-EXEC-006", "process stdin is missing"))?;
        let maximum_stdin = required_usize(node, "maximum_stdin_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        if input.bytes.len() > maximum_stdin {
            return Err(runtime_error(
                "CND-EXEC-006",
                "process stdin exceeds its exact byte ceiling",
            ));
        }
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
        let mut command = Command::new(executable);
        command
            .arg(FIXTURE_ENTRYPOINT)
            .args(arguments)
            .env_clear()
            .envs(environment)
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            runtime_error("CND-EXEC-008", format!("process spawn failed: {error}"))
        })?;
        if node.config("cancellation") == Some("cancel-after-spawn") {
            child.kill().map_err(|error| {
                runtime_error(
                    "CND-EXEC-011",
                    format!("forced process termination failed: {error}"),
                )
            })?;
            child.wait().map_err(|error| {
                runtime_error(
                    "CND-EXEC-011",
                    format!("process cleanup wait failed: {error}"),
                )
            })?;
            return Err(runtime_error(
                "CND-EXEC-009",
                "process cancelled after spawn; forced termination and wait completed",
            ));
        }

        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| runtime_error("CND-EXEC-008", "process stdin pipe is unavailable"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| runtime_error("CND-EXEC-008", "process stdout pipe is unavailable"))?;
        let child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| runtime_error("CND-EXEC-008", "process stderr pipe is unavailable"))?;
        let maximum_stdout = required_usize(node, "maximum_stdout_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let maximum_stderr = required_usize(node, "maximum_stderr_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let deadline_millis = required_u64(node, "deadline_ticks")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let started = Instant::now();
        let (stdout, stderr, timed_out) = thread::scope(|scope| {
            let input = input.bytes.as_slice();
            let writer = scope.spawn(move || {
                let mut child_stdin = child_stdin;
                child_stdin.write_all(input)?;
                child_stdin.flush()
            });
            let stdout_reader = scope.spawn(move || read_bounded(child_stdout, maximum_stdout));
            let stderr_reader = scope.spawn(move || read_bounded(child_stderr, maximum_stderr));
            let mut timed_out = false;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if started.elapsed() < Duration::from_millis(deadline_millis) => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Ok(None) => {
                        timed_out = true;
                        child.kill().map_err(|error| {
                            runtime_error(
                                "CND-EXEC-011",
                                format!("deadline termination failed: {error}"),
                            )
                        })?;
                        child.wait().map_err(|error| {
                            runtime_error(
                                "CND-EXEC-011",
                                format!("deadline cleanup wait failed: {error}"),
                            )
                        })?;
                        break;
                    }
                    Err(error) => {
                        return Err(runtime_error(
                            "CND-EXEC-011",
                            format!("process wait observation failed: {error}"),
                        ));
                    }
                }
            }
            writer
                .join()
                .map_err(|_| runtime_error("CND-EXEC-011", "stdin writer panicked"))?
                .map_err(|error| {
                    runtime_error("CND-EXEC-011", format!("stdin write failed: {error}"))
                })?;
            let stdout = stdout_reader
                .join()
                .map_err(|_| runtime_error("CND-EXEC-011", "stdout reader panicked"))?
                .map_err(|error| {
                    runtime_error("CND-EXEC-011", format!("stdout read failed: {error}"))
                })?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| runtime_error("CND-EXEC-011", "stderr reader panicked"))?
                .map_err(|error| {
                    runtime_error("CND-EXEC-011", format!("stderr read failed: {error}"))
                })?;
            Ok::<_, RuntimeError>((stdout, stderr, timed_out))
        })?;
        if timed_out {
            return Err(runtime_error(
                "CND-EXEC-012",
                "process deadline expired; forced termination and wait completed",
            ));
        }
        if stdout.len() > maximum_stdout {
            return Err(runtime_error(
                "CND-EXEC-013",
                "process stdout exceeded its exact byte ceiling",
            ));
        }
        if stderr.len() > maximum_stderr {
            return Err(runtime_error(
                "CND-EXEC-014",
                "process stderr exceeded its exact byte ceiling",
            ));
        }
        let status = child
            .try_wait()
            .map_err(|error| runtime_error("CND-EXEC-011", format!("exit read failed: {error}")))?
            .ok_or_else(|| runtime_error("CND-EXEC-011", "process exit was not observed"))?;
        if !status.success() {
            return Err(runtime_error(
                "CND-EXEC-015",
                format!("process exited unsuccessfully: {status}"),
            ));
        }
        Ok(vec![
            Value {
                value_type: contract.outputs[0].value_type,
                bytes: stdout,
            },
            Value {
                value_type: contract.outputs[1].value_type,
                bytes: stderr,
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
}
