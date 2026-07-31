//! Allocator-free semantics for the optional hosted process exec boundary.
//!
//! A command names an executable resource and literal arguments. It is never
//! shell text, never performs ambient `PATH` lookup, and carries no authority
//! by itself. Provider, host, executable artifact, grant, lease, working
//! resource, environment policy, and finite limits belong in the exact plan.

/// Maximum literal arguments in the deterministic reference profile.
pub const PROCESS_MAX_ARGUMENTS: usize = 16;
/// Maximum UTF-8 bytes in one literal argument.
pub const PROCESS_MAX_ARGUMENT_BYTES: usize = 256;
/// Maximum explicit environment additions.
pub const PROCESS_MAX_ENVIRONMENT: usize = 16;
/// Maximum UTF-8 bytes in one environment name.
pub const PROCESS_MAX_ENVIRONMENT_NAME_BYTES: usize = 64;
/// Maximum bytes in one environment value.
pub const PROCESS_MAX_ENVIRONMENT_VALUE_BYTES: usize = 512;
/// Maximum bytes accepted on stdin or retained per output stream.
pub const PROCESS_MAX_STREAM_BYTES: usize = 65_536;
/// Maximum bytes in one process stream chunk.
pub const PROCESS_MAX_CHUNK_BYTES: usize = 4_096;
/// Maximum deterministic lifecycle events.
pub const PROCESS_MAX_EVIDENCE_EVENTS: usize = 128;

/// One explicit environment addition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentAddition<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
    pub sensitive: bool,
}

/// Exact command meaning before a host binds executable resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecCommand<'a> {
    pub program_resource: &'a str,
    pub argv: &'a [&'a str],
    pub environment: &'a [EnvironmentAddition<'a>],
    pub working_resource: Option<&'a str>,
}

/// What the provider does with stdin after the finite authored stream closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StdinClosePolicy {
    CloseAfterInput,
}

/// Finite cancellation escalation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminationPolicy {
    pub graceful_signal: u8,
    pub graceful_ticks: u64,
    pub forced_ticks: u64,
}

/// Plan-visible finite process resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecLimits {
    pub maximum_stdin_bytes: usize,
    pub maximum_stdout_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub maximum_chunk_bytes: usize,
    pub maximum_pending_operations: usize,
    pub maximum_processes: usize,
    pub maximum_child_processes: usize,
    pub maximum_descriptors: usize,
    pub maximum_environment_bytes: usize,
    pub maximum_work: usize,
    pub maximum_evidence_events: usize,
}

/// One exact bounded exec request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecRequest<'a> {
    pub command: ExecCommand<'a>,
    pub stdin_close: StdinClosePolicy,
    pub deadline_ticks: u64,
    pub termination: TerminationPolicy,
    pub limits: ExecLimits,
}

/// Deterministic fake programs used by the reference provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeProgram {
    Echo,
    IndependentStreams,
    Exit(i32),
    Signal(u8),
    SpawnFailure,
    IgnoreGracefulTermination,
}

/// Which bounded output overflowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Typed terminal outcome; this is lifecycle evidence, not another byte port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecTerminal {
    Exited(i32),
    Signaled(u8),
    Cancelled { forced: bool },
    DeadlineExceeded { forced: bool },
    OutputOverflow(OutputStream),
}

/// One normalized lifecycle event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecEventKind {
    SpawnCommitted,
    SpawnFailed,
    StdinChunk,
    StdinClosed,
    StdoutChunk,
    StderrChunk,
    GracefulSignal,
    ForcedTermination,
    Exited,
    CleanupComplete,
}

/// One exact ordered event emitted by the deterministic provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecEvent {
    pub sequence: u16,
    pub tick: u64,
    pub kind: ExecEventKind,
    pub bytes: usize,
}

/// Finite deterministic execution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecResult {
    pub terminal: ExecTerminal,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub events: usize,
    pub cleanup_complete: bool,
}

/// Caller-owned bounded buffers for one deterministic execution.
pub struct FakeExecBuffers<'a> {
    pub stdin: &'a [u8],
    pub stdout: &'a mut [u8],
    pub stderr: &'a mut [u8],
    pub events: &'a mut [ExecEvent],
}

/// Explicit deterministic cancellation/deadline injection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FakeExecControl {
    pub cancel_before_spawn: bool,
    pub cancel_after_spawn: bool,
    pub deadline_after_spawn: bool,
}

/// Stable process boundary failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecError {
    InvalidProgram,
    ArgumentBoundExceeded,
    EnvironmentBoundExceeded,
    InvalidEnvironmentName,
    InvalidLimits,
    InputOverflow,
    EvidenceOverflow,
    SpawnFailed,
    CancelledBeforeSpawn,
}

impl ExecError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidProgram => "CND-EXEC-001",
            Self::ArgumentBoundExceeded => "CND-EXEC-002",
            Self::EnvironmentBoundExceeded => "CND-EXEC-003",
            Self::InvalidEnvironmentName => "CND-EXEC-004",
            Self::InvalidLimits => "CND-EXEC-005",
            Self::InputOverflow => "CND-EXEC-006",
            Self::EvidenceOverflow => "CND-EXEC-007",
            Self::SpawnFailed => "CND-EXEC-008",
            Self::CancelledBeforeSpawn => "CND-EXEC-009",
        }
    }
}

/// Validate all command and resource bounds without performing an effect.
pub fn validate_exec_request(request: &ExecRequest<'_>) -> Result<(), ExecError> {
    if request.command.program_resource.is_empty()
        || request.command.program_resource.as_bytes().contains(&0)
    {
        return Err(ExecError::InvalidProgram);
    }
    if request.command.argv.len() > PROCESS_MAX_ARGUMENTS
        || request.command.argv.iter().any(|argument| {
            argument.len() > PROCESS_MAX_ARGUMENT_BYTES || argument.as_bytes().contains(&0)
        })
    {
        return Err(ExecError::ArgumentBoundExceeded);
    }
    if request.command.environment.len() > PROCESS_MAX_ENVIRONMENT {
        return Err(ExecError::EnvironmentBoundExceeded);
    }
    let mut environment_bytes = 0usize;
    for addition in request.command.environment {
        if addition.name.is_empty()
            || addition.name.len() > PROCESS_MAX_ENVIRONMENT_NAME_BYTES
            || addition.name.as_bytes().contains(&0)
            || addition.name.as_bytes().contains(&b'=')
        {
            return Err(ExecError::InvalidEnvironmentName);
        }
        if addition.value.len() > PROCESS_MAX_ENVIRONMENT_VALUE_BYTES || addition.value.contains(&0)
        {
            return Err(ExecError::EnvironmentBoundExceeded);
        }
        environment_bytes = environment_bytes
            .checked_add(addition.name.len() + addition.value.len())
            .ok_or(ExecError::EnvironmentBoundExceeded)?;
    }
    let limits = request.limits;
    if limits.maximum_stdin_bytes == 0
        || limits.maximum_stdin_bytes > PROCESS_MAX_STREAM_BYTES
        || limits.maximum_stdout_bytes == 0
        || limits.maximum_stdout_bytes > PROCESS_MAX_STREAM_BYTES
        || limits.maximum_stderr_bytes == 0
        || limits.maximum_stderr_bytes > PROCESS_MAX_STREAM_BYTES
        || limits.maximum_chunk_bytes == 0
        || limits.maximum_chunk_bytes > PROCESS_MAX_CHUNK_BYTES
        || limits.maximum_pending_operations == 0
        || limits.maximum_processes != 1
        || limits.maximum_child_processes != 0
        || limits.maximum_descriptors < 3
        || environment_bytes > limits.maximum_environment_bytes
        || limits.maximum_work == 0
        || limits.maximum_evidence_events == 0
        || limits.maximum_evidence_events > PROCESS_MAX_EVIDENCE_EVENTS
        || request.deadline_ticks == 0
        || request.termination.graceful_signal == 0
        || request.termination.graceful_ticks == 0
        || request.termination.forced_ticks == 0
    {
        return Err(ExecError::InvalidLimits);
    }
    Ok(())
}

/// Run the allocator-free deterministic provider.
///
/// Output bytes and evidence are written only into caller-supplied bounded
/// storage. `cancel_before_spawn` proves that cancellation can prevent the
/// effect; `cancel_after_spawn` and `deadline_after_spawn` exercise finite
/// cleanup after the spawn commit point.
pub fn run_fake_exec(
    request: &ExecRequest<'_>,
    program: FakeProgram,
    buffers: FakeExecBuffers<'_>,
    control: FakeExecControl,
) -> Result<ExecResult, ExecError> {
    validate_exec_request(request)?;
    if buffers.stdin.len() > request.limits.maximum_stdin_bytes {
        return Err(ExecError::InputOverflow);
    }
    let event_limit = buffers
        .events
        .len()
        .min(request.limits.maximum_evidence_events);
    let mut event_count = 0usize;
    let mut push = |kind, tick, bytes| {
        if event_count >= event_limit {
            return Err(ExecError::EvidenceOverflow);
        }
        buffers.events[event_count] = ExecEvent {
            sequence: event_count as u16,
            tick,
            kind,
            bytes,
        };
        event_count += 1;
        Ok(())
    };
    if control.cancel_before_spawn {
        return Err(ExecError::CancelledBeforeSpawn);
    }
    if program == FakeProgram::SpawnFailure {
        push(ExecEventKind::SpawnFailed, 0, 0)?;
        return Err(ExecError::SpawnFailed);
    }
    push(ExecEventKind::SpawnCommitted, 0, 0)?;
    if !buffers.stdin.is_empty() {
        push(ExecEventKind::StdinChunk, 1, buffers.stdin.len())?;
    }
    push(ExecEventKind::StdinClosed, 2, 0)?;

    let forced = program == FakeProgram::IgnoreGracefulTermination;
    let terminal = if control.cancel_after_spawn || control.deadline_after_spawn {
        push(
            ExecEventKind::GracefulSignal,
            3 + request.termination.graceful_ticks,
            0,
        )?;
        if forced {
            push(
                ExecEventKind::ForcedTermination,
                3 + request.termination.graceful_ticks + request.termination.forced_ticks,
                0,
            )?;
        }
        if control.deadline_after_spawn {
            ExecTerminal::DeadlineExceeded { forced }
        } else {
            ExecTerminal::Cancelled { forced }
        }
    } else {
        match program {
            FakeProgram::Echo => {
                if buffers.stdin.len() > buffers.stdout.len()
                    || buffers.stdin.len() > request.limits.maximum_stdout_bytes
                {
                    ExecTerminal::OutputOverflow(OutputStream::Stdout)
                } else {
                    buffers.stdout[..buffers.stdin.len()].copy_from_slice(buffers.stdin);
                    if !buffers.stdin.is_empty() {
                        push(ExecEventKind::StdoutChunk, 3, buffers.stdin.len())?;
                    }
                    ExecTerminal::Exited(0)
                }
            }
            FakeProgram::IndependentStreams => {
                let diagnostic = b"diagnostic";
                if buffers.stdin.len() > buffers.stdout.len()
                    || buffers.stdin.len() > request.limits.maximum_stdout_bytes
                {
                    ExecTerminal::OutputOverflow(OutputStream::Stdout)
                } else if diagnostic.len() > buffers.stderr.len()
                    || diagnostic.len() > request.limits.maximum_stderr_bytes
                {
                    ExecTerminal::OutputOverflow(OutputStream::Stderr)
                } else {
                    buffers.stdout[..buffers.stdin.len()].copy_from_slice(buffers.stdin);
                    buffers.stderr[..diagnostic.len()].copy_from_slice(diagnostic);
                    if !buffers.stdin.is_empty() {
                        push(ExecEventKind::StdoutChunk, 3, buffers.stdin.len())?;
                    }
                    push(ExecEventKind::StderrChunk, 4, diagnostic.len())?;
                    ExecTerminal::Exited(0)
                }
            }
            FakeProgram::Exit(code) => ExecTerminal::Exited(code),
            FakeProgram::Signal(signal) => ExecTerminal::Signaled(signal),
            FakeProgram::IgnoreGracefulTermination => ExecTerminal::Exited(0),
            FakeProgram::SpawnFailure => unreachable!(),
        }
    };
    let stdout_bytes = match terminal {
        ExecTerminal::Exited(0)
            if matches!(program, FakeProgram::Echo | FakeProgram::IndependentStreams) =>
        {
            buffers.stdin.len()
        }
        _ => 0,
    };
    let stderr_bytes =
        if terminal == ExecTerminal::Exited(0) && program == FakeProgram::IndependentStreams {
            b"diagnostic".len()
        } else {
            0
        };
    push(ExecEventKind::Exited, request.deadline_ticks, 0)?;
    push(ExecEventKind::CleanupComplete, request.deadline_ticks, 0)?;
    Ok(ExecResult {
        terminal,
        stdout_bytes,
        stderr_bytes,
        events: event_count,
        cleanup_complete: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(argv: &'a [&'a str]) -> ExecRequest<'a> {
        ExecRequest {
            command: ExecCommand {
                program_resource: "conduit.executable/test",
                argv,
                environment: &[],
                working_resource: None,
            },
            stdin_close: StdinClosePolicy::CloseAfterInput,
            deadline_ticks: 32,
            termination: TerminationPolicy {
                graceful_signal: 15,
                graceful_ticks: 2,
                forced_ticks: 2,
            },
            limits: ExecLimits {
                maximum_stdin_bytes: 64,
                maximum_stdout_bytes: 64,
                maximum_stderr_bytes: 64,
                maximum_chunk_bytes: 16,
                maximum_pending_operations: 3,
                maximum_processes: 1,
                maximum_child_processes: 0,
                maximum_descriptors: 3,
                maximum_environment_bytes: 0,
                maximum_work: 128,
                maximum_evidence_events: 16,
            },
        }
    }

    #[test]
    fn shell_metacharacters_are_literal_arguments() {
        let request = request(&["$(touch nope)", ";", "|"]);
        assert_eq!(validate_exec_request(&request), Ok(()));
    }

    #[test]
    fn stdout_and_stderr_remain_independent() {
        let request = request(&[]);
        let mut stdout = [0; 64];
        let mut stderr = [0; 64];
        let mut events = [ExecEvent {
            sequence: 0,
            tick: 0,
            kind: ExecEventKind::SpawnCommitted,
            bytes: 0,
        }; 16];
        let result = run_fake_exec(
            &request,
            FakeProgram::IndependentStreams,
            FakeExecBuffers {
                stdin: b"payload",
                stdout: &mut stdout,
                stderr: &mut stderr,
                events: &mut events,
            },
            FakeExecControl::default(),
        )
        .unwrap();
        assert_eq!(&stdout[..result.stdout_bytes], b"payload");
        assert_eq!(&stderr[..result.stderr_bytes], b"diagnostic");
        assert!(
            events[..result.events]
                .iter()
                .any(|event| event.kind == ExecEventKind::StdoutChunk)
        );
        assert!(
            events[..result.events]
                .iter()
                .any(|event| event.kind == ExecEventKind::StderrChunk)
        );
    }

    #[test]
    fn cancellation_escalates_and_cleans_up() {
        let request = request(&[]);
        let mut stdout = [0; 64];
        let mut stderr = [0; 64];
        let mut events = [ExecEvent {
            sequence: 0,
            tick: 0,
            kind: ExecEventKind::SpawnCommitted,
            bytes: 0,
        }; 16];
        let result = run_fake_exec(
            &request,
            FakeProgram::IgnoreGracefulTermination,
            FakeExecBuffers {
                stdin: b"stream",
                stdout: &mut stdout,
                stderr: &mut stderr,
                events: &mut events,
            },
            FakeExecControl {
                cancel_after_spawn: true,
                ..FakeExecControl::default()
            },
        )
        .unwrap();
        assert_eq!(result.terminal, ExecTerminal::Cancelled { forced: true });
        assert!(result.cleanup_complete);
        assert!(
            events[..result.events]
                .iter()
                .any(|event| event.kind == ExecEventKind::ForcedTermination)
        );
    }

    #[test]
    fn invalid_bounds_fail_before_spawn() {
        let mut request = request(&[]);
        request.limits.maximum_processes = 2;
        assert_eq!(
            validate_exec_request(&request),
            Err(ExecError::InvalidLimits)
        );
    }
}
