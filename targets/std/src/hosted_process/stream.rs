//! Demand-driven subprocess stdout. No worker drains ahead of the consumer.
use super::*;
#[cfg(test)]
pub(crate) mod observations;
use std::process::{ChildStderr, ChildStdin, ChildStdout};

#[derive(Clone, Debug)]
pub(crate) enum StreamFailure {
    StdoutBoundExceeded,
    Terminal(ProcessTerminal),
}
impl From<ProcessTerminal> for StreamFailure {
    fn from(value: ProcessTerminal) -> Self {
        Self::Terminal(value)
    }
}

pub(crate) struct ProcessStream {
    child: ChildGuard,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdin: Option<ChildStdin>,
    input: [u8; 1024],
    input_len: usize,
    sent: usize,
    stderr_bytes: usize,
    stderr_eof: bool,
    output_bytes: usize,
    maximum_output: usize,
    started: Instant,
    timeout: Duration,
    finished: bool,
    failure: Option<StreamFailure>,
}
impl ProcessStream {
    pub(crate) fn start(request: &ProcessRequest<'_>) -> Result<Self, ProcessError> {
        if !cfg!(unix) || !request.require_process_group {
            return Err(ProcessError::Unsupported(
                "incremental process requires Unix process retirement",
            ));
        }
        if !request.program.is_absolute()
            || request.stdin.len() > 1024
            || request.maximum_stdout_bytes > 1_323_044
            || request.maximum_stderr_bytes != 4096
            || request.timeout.is_zero()
            || request.timeout > Duration::from_secs(30)
        {
            return Err(ProcessError::InvalidRequest(
                "invalid incremental process bounds",
            ));
        }
        let mut command = Command::new(request.program);
        command
            .args(request.arguments)
            .env_clear()
            .envs(request.environment.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = ChildGuard(command.spawn().map_err(ProcessError::Launch)?, false);
        let stdout = child.0.stdout.take().expect("piped stdout");
        let stderr = child.0.stderr.take().expect("piped stderr");
        let stdin = child.0.stdin.take().expect("piped stdin");
        prepare_pipe(&stdout)
            .and_then(|()| prepare_pipe(&stderr))
            .and_then(|()| prepare_pipe(&stdin))
            .map_err(ProcessError::Launch)?;
        let mut input = [0; 1024];
        input[..request.stdin.len()].copy_from_slice(request.stdin);
        Ok(Self {
            child,
            stdout,
            stderr,
            stdin: Some(stdin),
            input,
            input_len: request.stdin.len(),
            sent: 0,
            stderr_bytes: 0,
            stderr_eof: false,
            output_bytes: 0,
            maximum_output: request.maximum_stdout_bytes,
            started: Instant::now(),
            timeout: request.timeout,
            finished: false,
            failure: None,
        })
    }
    pub(crate) fn read(
        &mut self,
        output: &mut [u8],
        cancelled: impl Fn() -> bool,
    ) -> Result<usize, StreamFailure> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if output.is_empty() {
            return Err(ProcessTerminal::ProviderLost("empty read buffer".into()).into());
        }
        if self.finished {
            return Ok(0);
        }
        loop {
            let failure = if cancelled() {
                Some(ProcessTerminal::Cancelled)
            } else if self.started.elapsed() >= self.timeout {
                Some(ProcessTerminal::TimedOut)
            } else {
                None
            };
            if let Some(failure) = failure {
                let failure = StreamFailure::Terminal(failure);
                self.failure = Some(failure.clone());
                self.retire();
                return Err(failure);
            }
            let result = self.poll(output);
            match result {
                Ok(Some(size)) => return Ok(size),
                Ok(None) => thread::sleep(POLL),
                Err(error) => {
                    self.failure = Some(error.clone());
                    self.retire();
                    return Err(error);
                }
            }
        }
    }
    fn poll(&mut self, output: &mut [u8]) -> Result<Option<usize>, StreamFailure> {
        let failure =
            |e: io::Error| StreamFailure::Terminal(ProcessTerminal::ProviderLost(e.to_string()));
        if let Some(input) = &mut self.stdin {
            match input.write(&self.input[self.sent..self.input_len]) {
                Ok(size) => {
                    self.sent += size;
                    if self.sent == self.input_len {
                        self.stdin.take();
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(failure(e)),
            }
        }
        let mut scratch = [0; 4096];
        match self.stderr.read(&mut scratch) {
            Ok(size) => {
                self.stderr_eof = size == 0;
                self.stderr_bytes += size;
                if self.stderr_bytes > 4096 {
                    return Err(
                        ProcessTerminal::ProviderLost("stderr bound exceeded".into()).into(),
                    );
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(failure(e)),
        }
        match self.stdout.read(output) {
            Ok(0) => match self.child.0.try_wait().map_err(failure)? {
                Some(status) => {
                    if !self.stderr_eof {
                        return Ok(None);
                    }
                    self.retire();
                    if status.success() {
                        Ok(Some(0))
                    } else {
                        Err(ProcessTerminal::Exited(status).into())
                    }
                }
                None => Ok(None),
            },
            Ok(size) => {
                self.output_bytes += size;
                if self.output_bytes > self.maximum_output {
                    return Err(StreamFailure::StdoutBoundExceeded);
                }
                #[cfg(test)]
                observations::emit(observations::Event {
                    pid: self.child.0.id(),
                    stdout_bytes: self.output_bytes,
                    reaped: None,
                });
                Ok(Some(size))
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(failure(e)),
        }
    }
    pub(crate) fn retire(&mut self) {
        if !self.finished {
            terminate(&mut self.child.0);
            let _reaped = self.child.0.wait().is_ok();
            #[cfg(test)]
            observations::emit(observations::Event {
                pid: self.child.0.id(),
                stdout_bytes: self.output_bytes,
                reaped: Some(_reaped),
            });
            self.child.1 = true;
            self.finished = true;
            self.stdin.take();
        }
    }
}
impl Drop for ProcessStream {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn start(script: &str, maximum: usize, timeout: Duration) -> ProcessStream {
        let arguments = [OsString::from("-c"), OsString::from(script)];
        ProcessStream::start(&ProcessRequest {
            program: Path::new("/bin/sh"),
            arguments: &arguments,
            environment: &[],
            stdin: b"",
            maximum_stdout_bytes: maximum,
            maximum_stderr_bytes: 4096,
            timeout,
            require_process_group: true,
        })
        .unwrap()
    }
    #[test]
    fn first_bytes_arrive_before_process_completion_and_consumer_drives_reads() {
        let mut stream = start(
            "printf first; sleep 1; printf last",
            9,
            Duration::from_secs(3),
        );
        let mut bytes = [0; 5];
        assert_eq!(stream.read(&mut bytes, || false).unwrap(), 5);
        assert_eq!(&bytes, b"first");
        assert!(stream.child.0.try_wait().unwrap().is_none());
        assert_eq!(stream.output_bytes, 5);
        let mut result = Vec::new();
        loop {
            let count = stream.read(&mut bytes, || false).unwrap();
            if count == 0 {
                break;
            }
            result.extend_from_slice(&bytes[..count]);
        }
        assert_eq!(result, b"last");
    }
    #[test]
    fn cancellation_and_timeout_reap_process_without_successful_eof() {
        for cancel in [true, false] {
            let mut stream = start("sleep 2", 8, Duration::from_millis(20));
            let result = stream.read(&mut [0; 8], || cancel);
            assert!(matches!(
                (&result, cancel),
                (
                    Err(StreamFailure::Terminal(ProcessTerminal::Cancelled)),
                    true
                ) | (
                    Err(StreamFailure::Terminal(ProcessTerminal::TimedOut)),
                    false
                )
            ));
            assert!(stream.finished && stream.child.1);
            assert!(stream.read(&mut [0; 8], || false).is_err());
        }
    }
    #[test]
    fn stdout_and_stderr_overflow_refuse_completion() {
        for (script, limit) in [
            ("printf 123456789", 8),
            ("head -c 5000 /dev/zero >&2; printf x", 8),
        ] {
            let mut stream = start(script, limit, Duration::from_secs(2));
            let mut refused = false;
            loop {
                match stream.read(&mut [0; 16], || false) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) => {
                        if script.starts_with("printf") {
                            assert!(matches!(error, StreamFailure::StdoutBoundExceeded));
                        } else {
                            assert!(matches!(
                                error,
                                StreamFailure::Terminal(ProcessTerminal::ProviderLost(_))
                            ));
                        }
                        refused = true;
                        break;
                    }
                }
            }
            assert!(refused);
        }
    }
}
