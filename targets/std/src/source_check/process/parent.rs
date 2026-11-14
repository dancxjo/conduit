//! Parent-owned bounded preparation using the existing process polling driver.
use super::*;
use crate::hosted_process::{run_process_with_input_bound, ProcessRequest, ProcessTerminal};
use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Debug)]
pub enum SourceCheckProcessFailure {
    Bounds,
    InputBounds,
    Cancelled,
    Deadline,
    Launch(String),
    ProviderLost(String),
    WorkerExited {
        code: Option<i32>,
        diagnostic: Vec<u8>,
    },
    OutputBounds,
    MalformedOutput,
    ExecutableIdentity,
    ExecutableBounds,
}
/// Descriptive canonical output only. This is not a Native admission witness,
/// compiler owner, Source grant or resource-retirement capability.
pub struct SourceCheckProcessOutput {
    bytes: Vec<u8>,
    request: SourceCheckRequest,
}
impl SourceCheckProcessOutput {
    /// The exact request sent to the selected child; descriptive, not a grant.
    pub fn request(&self) -> &SourceCheckRequest {
        &self.request
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
/// Configuration comes from independently authorized Host construction, never a
/// Plot input. Calling this helper creates no Ready offer or reservation; the
/// Root must own the actual selected executable and admit parent/child backing.
pub struct SourceCheckProcessWorker {
    pub(super) executable: PathBuf,
    limits: SourceCheckProcessLimits,
    pub(super) deadline: Duration,
    maximum_startup_bytes: usize,
    maximum_reply_bytes: usize,
    pub(super) executable_identity: Option<super::executable::ExecutableIdentity>,
}
impl SourceCheckProcessWorker {
    pub fn new(
        executable: PathBuf,
        limits: SourceCheckProcessLimits,
        deadline: Duration,
        maximum_startup_bytes: usize,
    ) -> Result<Self, SourceCheckProcessFailure> {
        if !executable.is_absolute()
            || deadline.is_zero()
            || deadline > Duration::from_secs(30)
            || maximum_startup_bytes == 0
            || maximum_startup_bytes as u64 > MAXIMUM_INPUT_BYTES
            || limits.validate().is_err()
        {
            return Err(SourceCheckProcessFailure::Bounds);
        }
        Ok(Self {
            executable,
            limits,
            deadline,
            maximum_startup_bytes,
            maximum_reply_bytes: MAXIMUM_REPLY_BYTES,
            executable_identity: None,
        })
    }
    /// Restrict retained child output to the checked provider's reply envelope.
    pub fn with_reply_bound(mut self, bytes: usize) -> Result<Self, SourceCheckProcessFailure> {
        if bytes == 0 || bytes > MAXIMUM_REPLY_BYTES {
            return Err(SourceCheckProcessFailure::Bounds);
        }
        self.maximum_reply_bytes = bytes;
        Ok(self)
    }
    /// One child at a time through this owner. Polling kills the child's group
    /// on Stop/deadline; the existing driver waits/reaps before returning.
    /// No output is published from failed, oversized or cancelled work.
    pub fn prepare(
        &mut self,
        input: &SourceCheckProcessInput,
        control: &crate::RunControl,
    ) -> Result<SourceCheckProcessOutput, SourceCheckProcessFailure> {
        if control.stop_requested() {
            return Err(SourceCheckProcessFailure::Cancelled);
        }
        let started = Instant::now();
        let request = &input.request;
        let labels = [
            &input.owner,
            &request.snapshot.owner,
            &request.snapshot.sha256,
            &request.snapshot.source_document_id,
            &request.context.grammar_edition,
            &request.context.catalog,
            &request.context.kinds_fores,
            &request.context.types_laws,
            &request.context.backs,
            &request.entry,
        ];
        if input.source.len() > conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES
            || labels
                .iter()
                .any(|label| label.len() > conduit_plot::MAX_CHECK_LABEL_BYTES)
        {
            return Err(SourceCheckProcessFailure::InputBounds);
        }
        let mut startup = BoundedStartup {
            bytes: Vec::with_capacity(self.maximum_startup_bytes),
            maximum: self.maximum_startup_bytes,
        };
        serde_json::to_writer(&mut startup, input)
            .map_err(|_| SourceCheckProcessFailure::InputBounds)?;
        self.observe_executable(control, started)?;
        let remaining = self
            .deadline
            .checked_sub(started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(SourceCheckProcessFailure::Deadline)?;
        let arguments = [
            OsString::from("source-check-worker"),
            OsString::from("--address-space-bytes"),
            self.limits.address_space_bytes.to_string().into(),
            OsString::from("--cpu-seconds"),
            self.limits.cpu_seconds.to_string().into(),
        ];
        let report = run_process_with_input_bound(
            &ProcessRequest {
                program: &self.executable,
                arguments: &arguments,
                environment: &[],
                stdin: &startup.bytes,
                maximum_stdout_bytes: self.maximum_reply_bytes,
                maximum_stderr_bytes: 4096,
                timeout: remaining,
                require_process_group: true,
            },
            self.maximum_startup_bytes,
            || control.stop_requested(),
        )
        .map_err(|error| SourceCheckProcessFailure::Launch(format!("{error:?}")))?;
        match report.terminal {
            ProcessTerminal::Cancelled => return Err(SourceCheckProcessFailure::Cancelled),
            ProcessTerminal::TimedOut => return Err(SourceCheckProcessFailure::Deadline),
            ProcessTerminal::ProviderLost(reason) => {
                return Err(SourceCheckProcessFailure::ProviderLost(reason))
            }
            ProcessTerminal::Exited(status) if !status.success() => {
                return Err(SourceCheckProcessFailure::WorkerExited {
                    code: status.code(),
                    diagnostic: report.stderr.retained,
                })
            }
            ProcessTerminal::Exited(_) => {}
        }
        // A Stop observed before publication discards even a completed reply.
        if control.stop_requested() {
            return Err(SourceCheckProcessFailure::Cancelled);
        }
        if report.stdout.observed_bytes > self.maximum_reply_bytes as u64 {
            return Err(SourceCheckProcessFailure::OutputBounds);
        }
        conduit_core::validate_canonical_structured_value(&report.stdout.retained)
            .map_err(|_| SourceCheckProcessFailure::MalformedOutput)?;
        Ok(SourceCheckProcessOutput {
            bytes: report.stdout.retained,
            request: input.request.clone(),
        })
    }
}
struct BoundedStartup {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for BoundedStartup {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
