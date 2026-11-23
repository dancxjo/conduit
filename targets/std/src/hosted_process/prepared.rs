//! One-shot owned preparation for the existing polling/retirement loop.
use super::*;
pub(crate) struct PreparedProcess {
    pub(super) command: Command,
    pub(super) diagnostic: Option<String>,
    reserved_backing_bytes: usize,
    pub(super) out: CapturedOutput,
    pub(super) err: CapturedOutput,
    pub(super) maximum_input_bytes: usize,
    pub(super) input_is_empty: bool,
    pub(super) maximum_stdout_bytes: usize,
    pub(super) maximum_stderr_bytes: usize,
    pub(super) timeout: Duration,
}
impl PreparedProcess {
    pub(super) fn validate(
        request: &ProcessRequest<'_>,
        maximum_input_bytes: usize,
    ) -> Result<(), ProcessError> {
        Self::validate_with_output_bound(request, maximum_input_bytes, 131_116)
    }
    pub(super) fn validate_with_output_bound(
        request: &ProcessRequest<'_>,
        maximum_input_bytes: usize,
        maximum_stdout_bytes: usize,
    ) -> Result<(), ProcessError> {
        if maximum_input_bytes > 8 * 1024 * 1024
            || (!cfg!(unix) && maximum_input_bytes > 256)
            || !request.program.is_absolute()
            || request.stdin.len() > maximum_input_bytes
            || maximum_stdout_bytes > 16 * 1024 * 1024
            || request.maximum_stdout_bytes > maximum_stdout_bytes
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
        Ok(())
    }
    /// Fixed-environment funded profile. The caller reserves the descriptor
    /// before this preparation, including the finite spawn residual.
    pub(crate) fn prepare_funded(
        request: &ProcessRequest<'_>,
        maximum_input_bytes: usize,
    ) -> Result<Self, ProcessError> {
        if !cfg!(conduit_prepared_process_layout) {
            return Err(ProcessError::Unsupported(env!(
                "CONDUIT_PREPARED_PROCESS_LAYOUT"
            )));
        }
        if request.environment.len() != 1
            || request.environment[0].0 != "LC_ALL"
            || request.environment[0].1 != "C"
            || !request.require_process_group
        {
            return Err(ProcessError::InvalidRequest(
                "funded process requires fixed LC_ALL=C and process group",
            ));
        }
        // The qualified producer recipe uses exact CString(len + 1).
        // std's malformed-NUL fallback has a different extent; refuse it
        // before constructing any owned Command instead of charging a guess.
        if request.program.as_os_str().as_encoded_bytes().contains(&0)
            || request
                .arguments
                .iter()
                .any(|arg| arg.as_encoded_bytes().contains(&0))
        {
            return Err(ProcessError::InvalidRequest(
                "funded process program/argument contains NUL",
            ));
        }
        let layout = super::layout::process_layout(
            request.program.as_os_str().as_encoded_bytes().len(),
            request
                .arguments
                .iter()
                .map(|value| value.as_encoded_bytes().len()),
            request.maximum_stdout_bytes,
            request.maximum_stderr_bytes,
        )
        .map_err(ProcessError::Unsupported)?;
        let mut prepared = Self::prepare(request, maximum_input_bytes)?;
        prepared.reserved_backing_bytes = layout
            .retained_bytes
            .checked_add(layout.spawn_bytes)
            .ok_or(ProcessError::InvalidRequest("process backing overflow"))?;
        prepared.diagnostic = Some(String::with_capacity(super::layout::DIAGNOSTIC_BYTES));
        Ok(prepared)
    }
    pub(crate) fn reserved_backing_bytes(&self) -> usize {
        self.reserved_backing_bytes
    }
    /// Creates no child and performs no platform effect.
    pub(crate) fn prepare(
        request: &ProcessRequest<'_>,
        maximum_input_bytes: usize,
    ) -> Result<Self, ProcessError> {
        Self::prepare_with_output_bound(request, maximum_input_bytes, 131_116)
    }
    pub(super) fn prepare_with_output_bound(
        request: &ProcessRequest<'_>,
        maximum_input_bytes: usize,
        maximum_stdout_bytes: usize,
    ) -> Result<Self, ProcessError> {
        Self::validate_with_output_bound(request, maximum_input_bytes, maximum_stdout_bytes)?;
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
        Ok(Self {
            command,
            diagnostic: None,
            reserved_backing_bytes: 0,
            out: CapturedOutput {
                retained: Vec::with_capacity(request.maximum_stdout_bytes),
                observed_bytes: 0,
            },
            err: CapturedOutput {
                retained: Vec::with_capacity(request.maximum_stderr_bytes),
                observed_bytes: 0,
            },
            maximum_input_bytes,
            input_is_empty: request.stdin.is_empty(),
            maximum_stdout_bytes: request.maximum_stdout_bytes,
            maximum_stderr_bytes: request.maximum_stderr_bytes,
            timeout: request.timeout,
        })
    }
}

/// A finite prepared diagnostic is consumed by the first provider failure.
pub(super) fn provider_error(
    buffer: &mut Option<String>,
    message: std::fmt::Arguments<'_>,
) -> String {
    use std::fmt::Write;
    let Some(slot) = buffer.as_mut() else {
        return message.to_string();
    };
    let mut text = std::mem::take(slot);
    if text.capacity() == 0 {
        return text;
    }
    struct Writer<'a>(&'a mut String);
    impl Write for Writer<'_> {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            if value.len() > self.0.capacity() - self.0.len() {
                return Err(std::fmt::Error);
            }
            self.0.push_str(value);
            Ok(())
        }
    }
    if Writer(&mut text).write_fmt(message).is_err() {
        text.clear();
        text.push_str("bounded process provider diagnostic exceeded prepared extent");
    }
    text
}
