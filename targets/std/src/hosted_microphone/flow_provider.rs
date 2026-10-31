//! Nonblocking finite capture custody. Every poll does at most one bounded read
//! per pipe. A retained frame must be consumed before polling again; pressure
//! fails explicitly instead of dropping data or growing storage.
use super::capture_flow::{CaptureSampleClock, MicrophoneFlowLimits, PreparedPcmCapture};
use super::*;
use std::io::PipeReader;
use std::process::ExitStatus;

pub struct PreparedMicrophoneFlow {
    frames: PreparedPcmCapture,
    command: Command,
    executable: PathBuf,
    executable_sha256: String,
    timeout: Duration,
    diagnostics: Box<[u8; MAXIMUM_DIAGNOSTIC_BYTES]>,
    output_pipe: PipeReader,
    diagnostic_pipe: PipeReader,
}
impl AlsaMicrophoneAdapter {
    /// Preparation does not open the microphone or execute a provider.
    pub fn prepare_pcm_flow(
        &self,
        block_frames: u16,
    ) -> Result<PreparedMicrophoneFlow, MicrophoneFailure> {
        self.prepare_pcm_flow_for(self.limits.capture_milliseconds, block_frames)
    }
    pub fn prepare_pcm_flow_for(
        &self,
        milliseconds: u32,
        block_frames: u16,
    ) -> Result<PreparedMicrophoneFlow, MicrophoneFailure> {
        if milliseconds == 0 || milliseconds > self.limits.capture_milliseconds {
            return Err(MicrophoneFailure::InvalidLimits);
        }
        let frames = PreparedPcmCapture::prepare(
            MicrophoneFlowLimits {
                total_frames: capture_frames(milliseconds),
                block_frames,
            },
            CaptureSampleClock::reserve()?,
        )?;
        let mut command = self.capture_process_for(capture_frames(milliseconds));
        // Setup diagnostics form a checked provider protocol, independent of locale.
        command.env("LC_ALL", "C");
        command.args([
            "--verbose",
            "--fatal-errors",
            "--period-size",
            &block_frames.to_string(),
            "--buffer-size",
            &(u32::from(block_frames) * 4).to_string(),
        ]);
        let (output_pipe, output_writer) =
            std::io::pipe().map_err(|_| MicrophoneFailure::ReadFailed)?;
        let (diagnostic_pipe, diagnostic_writer) =
            std::io::pipe().map_err(|_| MicrophoneFailure::ReadFailed)?;
        prepare_pipe(&output_pipe)?;
        prepare_pipe(&diagnostic_pipe)?;
        command.stdout(output_writer).stderr(diagnostic_writer);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        Ok(PreparedMicrophoneFlow {
            frames,
            command,
            executable: self.discovery.executable.clone(),
            executable_sha256: self.discovery.executable_sha256.clone(),
            timeout: self.limits.timeout,
            diagnostics: Box::new([0; MAXIMUM_DIAGNOSTIC_BYTES]),
            output_pipe,
            diagnostic_pipe,
        })
    }
}
impl PreparedMicrophoneFlow {
    pub fn limits(&self) -> MicrophoneFlowLimits {
        self.frames.limits()
    }
    pub fn clock_id(&self) -> u64 {
        self.frames.clock().id()
    }
    pub fn start(mut self, cancelled: bool) -> Result<MicrophoneFlow, MicrophoneFailure> {
        if cancelled {
            return Err(MicrophoneFailure::Cancelled);
        }
        let started = Instant::now();
        let current = exact_file(&self.executable)?;
        if current != self.executable || digest_file(&current)? != self.executable_sha256 {
            return Err(MicrophoneFailure::SelectionDrift);
        }
        if started.elapsed() >= self.timeout {
            return Err(MicrophoneFailure::Timeout);
        }
        let child = self
            .command
            .spawn()
            .map_err(|_| MicrophoneFailure::SpawnFailed)?;
        let flow = MicrophoneFlow {
            child,
            output: self.output_pipe,
            diagnostics: self.diagnostic_pipe,
            frames: self.frames,
            diagnostic_bytes: self.diagnostics,
            diagnostic_count: 0,
            started,
            timeout: self.timeout,
            stdout_eof: false,
            stderr_eof: false,
            status: None,
            active: true,
            complete: false,
            frame_announced: false,
            format_confirmed: false,
        };

        // Drop closes/retires an unsuccessfully prepared live session as well.
        flow.check_deadline(false)?;
        Ok(flow)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneFlowPoll {
    Pending,
    Frame,
    Complete,
}

pub struct MicrophoneFlow {
    child: Child,
    output: PipeReader,
    diagnostics: PipeReader,
    frames: PreparedPcmCapture,
    diagnostic_bytes: Box<[u8; MAXIMUM_DIAGNOSTIC_BYTES]>,
    diagnostic_count: usize,
    started: Instant,
    timeout: Duration,
    stdout_eof: bool,
    stderr_eof: bool,
    status: Option<ExitStatus>,
    active: bool,
    complete: bool,
    frame_announced: bool,
    format_confirmed: bool,
}
impl MicrophoneFlow {
    pub fn frame(&self) -> Option<&[u8]> {
        if self.active && self.frame_announced {
            self.frames.frame()
        } else {
            None
        }
    }
    pub fn release_frame(&mut self) -> Result<(), MicrophoneFailure> {
        self.frames.release_frame()?;
        self.frame_announced = false;
        Ok(())
    }
    pub fn storage_capacity(&self) -> usize {
        self.frames.storage_capacity() + MAXIMUM_DIAGNOSTIC_BYTES
    }
    pub fn clock_id(&self) -> u64 {
        self.frames.clock().id()
    }
    fn check_deadline(&self, cancelled: bool) -> Result<(), MicrophoneFailure> {
        if cancelled {
            return Err(MicrophoneFailure::Cancelled);
        }
        if self.started.elapsed() >= self.timeout {
            return Err(MicrophoneFailure::Timeout);
        }
        Ok(())
    }
    pub fn poll(&mut self, cancelled: bool) -> Result<MicrophoneFlowPoll, MicrophoneFailure> {
        let result = self.poll_inner(cancelled);
        if result.is_err() {
            self.retire();
        }
        result
    }
    fn poll_inner(&mut self, cancelled: bool) -> Result<MicrophoneFlowPoll, MicrophoneFailure> {
        if self.complete {
            return Ok(MicrophoneFlowPoll::Complete);
        }
        self.check_deadline(cancelled)?;
        if !self.active {
            return Err(MicrophoneFailure::ProviderLost);
        }
        if self.frame_announced {
            return Err(MicrophoneFailure::CaptureOverrun);
        }
        let mut scratch = [0; 4096];
        // Read diagnostics first; retain the exact bounded prefix for format
        // validation, including warnings split across several provider writes.
        if !self.stderr_eof {
            match self.diagnostics.read(&mut scratch) {
                Ok(0) => self.stderr_eof = true,
                Ok(count) => {
                    if count > MAXIMUM_DIAGNOSTIC_BYTES - self.diagnostic_count {
                        return Err(MicrophoneFailure::OutputOverflow);
                    }
                    self.diagnostic_bytes[self.diagnostic_count..self.diagnostic_count + count]
                        .copy_from_slice(&scratch[..count]);
                    self.diagnostic_count += count;
                    if self.diagnostic_bytes[..self.diagnostic_count]
                        .windows(b"overrun".len())
                        .any(|part| part == b"overrun")
                        || self.diagnostic_bytes[..self.diagnostic_count]
                            .windows(b"attempting recover".len())
                            .any(|part| part == b"attempting recover")
                    {
                        return Err(MicrophoneFailure::CaptureOverrun);
                    }
                    if self.diagnostic_bytes[..self.diagnostic_count]
                        .windows(b"rate is not accurate".len())
                        .any(|part| part == b"rate is not accurate")
                    {
                        return Err(MicrophoneFailure::InexactFormat);
                    }
                }
                Err(error) if pending(&error) => {}
                Err(_) => return Err(MicrophoneFailure::ReadFailed),
            }
        }
        self.format_confirmed =
            super::flow_format::confirmed(&self.diagnostic_bytes[..self.diagnostic_count])?;
        if !self.stdout_eof && self.frames.frame().is_none() {
            let count = self.frames.remaining_block_bytes().clamp(1, scratch.len());
            match self.output.read(&mut scratch[..count]) {
                Ok(0) => self.stdout_eof = true,
                Ok(count) => self.frames.accept(&scratch[..count])?,
                Err(error) if pending(&error) => {}
                Err(_) => return Err(MicrophoneFailure::ReadFailed),
            }
        }
        if self.status.is_none() {
            self.status = self
                .child
                .try_wait()
                .map_err(|_| MicrophoneFailure::ProviderLost)?;
        }
        if self.status.is_some_and(|status| !status.success()) {
            return Err(MicrophoneFailure::ProviderLost);
        }
        self.check_deadline(cancelled)?;
        if self.stderr_eof && !self.format_confirmed {
            return Err(MicrophoneFailure::UnconfirmedFormat);
        }
        if self.stdout_eof {
            self.frames.finish()?;
        }
        if self.frames.frame().is_some() && self.format_confirmed {
            self.frame_announced = true;
            return Ok(MicrophoneFlowPoll::Frame);
        }
        if self.stdout_eof && self.stderr_eof && self.status.is_some() {
            self.frames.finish()?;
            self.retire();
            self.complete = true;
            return Ok(MicrophoneFlowPoll::Complete);
        }
        Ok(MicrophoneFlowPoll::Pending)
    }
    fn retire(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        retire_provider(&mut self.child);
    }
}
impl Drop for MicrophoneFlow {
    fn drop(&mut self) {
        self.retire();
    }
}
fn pending(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
    )
}

fn retire_provider(child: &mut Child) {
    #[cfg(unix)]
    {
        // This is the process group created for this provider, not the caller.
        let group = child.id() as i32;
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    stop(child);
}

#[cfg(target_os = "linux")]
fn prepare_pipe(pipe: &PipeReader) -> Result<(), MicrophoneFailure> {
    use std::os::fd::AsRawFd;
    provider_io::nonblocking(pipe)?;
    // Kernel pipe storage is observed and bounded before provider acquisition.
    let bytes = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETPIPE_SZ) };
    if bytes <= 0 || bytes > 65_536 {
        return Err(MicrophoneFailure::InvalidLimits);
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn prepare_pipe(_: &PipeReader) -> Result<(), MicrophoneFailure> {
    Err(MicrophoneFailure::InvalidProvider)
}
