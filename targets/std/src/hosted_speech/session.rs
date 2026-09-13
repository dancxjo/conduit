//! One resumable Piper child process with a single-block pull boundary.

use super::{PiperFailure, PiperLimits, PiperSynthesisReceipt};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Child, ChildStderr, ChildStdout};
use std::time::{Duration, Instant};

const MAXIMUM_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const READ_BYTES: usize = 4 * 1024;
const PIPER_CLOCK_ID: u64 = 0x5049_5045_5201;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiperSynthesisStep<'a> {
    Block(&'a [u8]),
    Complete(PiperSynthesisReceipt),
}

pub(super) struct PiperSession {
    limits: PiperLimits,
    sample_rate_hz: u32,
    child: Option<PiperChild>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    started: Option<Instant>,
    stdout_closed: bool,
    stderr_closed: bool,
    exit_success: Option<bool>,
    raw: Vec<u8>,
    encoded: Vec<u8>,
    diagnostics: Vec<u8>,
    frames: u32,
    blocks: u16,
    text_sha256: String,
    pcm_sha256: String,
    pcm_digest: Sha256,
}

impl PiperSession {
    pub(super) fn is_active(&self) -> bool {
        self.child.is_some()
    }

    pub(super) fn new(limits: PiperLimits, sample_rate_hz: u32) -> Self {
        let block_bytes = usize::from(conduit_std_offers::PIPER_FRAMES_PER_BLOCK) * 2;
        Self {
            limits,
            sample_rate_hz,
            child: None,
            stdout: None,
            stderr: None,
            started: None,
            stdout_closed: false,
            stderr_closed: false,
            exit_success: None,
            // At most block_bytes - 1 bytes are retained before one read.
            raw: Vec::with_capacity(block_bytes + READ_BYTES),
            encoded: Vec::with_capacity(conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + block_bytes),
            diagnostics: Vec::with_capacity(MAXIMUM_DIAGNOSTIC_BYTES),
            frames: 0,
            blocks: 0,
            text_sha256: String::with_capacity(64),
            pcm_sha256: String::with_capacity(64),
            pcm_digest: Sha256::new(),
        }
    }

    pub(super) fn begin(&mut self, child: Child, text: &str) -> Result<(), PiperFailure> {
        if self.child.is_some() {
            return Err(PiperFailure::ProviderBusy);
        }
        let mut child = PiperChild(child);
        let mut stdin = child.0.stdin.take().ok_or(PiperFailure::SpawnFailed)?;
        if stdin
            .write_all(text.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .is_err()
        {
            return match child.0.try_wait() {
                Ok(Some(_)) => Err(PiperFailure::ProviderLost),
                _ => Err(PiperFailure::WriteFailed),
            };
        }
        drop(stdin);
        let stdout = child.0.stdout.take().ok_or(PiperFailure::SpawnFailed)?;
        let stderr = child.0.stderr.take().ok_or(PiperFailure::SpawnFailed)?;
        nonblocking(stdout.as_raw_fd())?;
        nonblocking(stderr.as_raw_fd())?;

        self.raw.clear();
        self.encoded.clear();
        self.diagnostics.clear();
        self.frames = 0;
        self.blocks = 0;
        self.text_sha256.clear();
        if self.text_sha256.capacity() < 64 {
            self.text_sha256.reserve_exact(64);
        }
        self.pcm_sha256.clear();
        if self.pcm_sha256.capacity() < 64 {
            self.pcm_sha256.reserve_exact(64);
        }
        use core::fmt::Write as _;
        write!(self.text_sha256, "{:x}", Sha256::digest(text.as_bytes()))
            .map_err(|_| PiperFailure::InvalidProvider)?;
        self.pcm_digest = Sha256::new();
        self.stdout_closed = false;
        self.stderr_closed = false;
        self.exit_success = None;
        self.started = Some(Instant::now());
        self.stdout = Some(stdout);
        self.stderr = Some(stderr);
        self.child = Some(child);
        Ok(())
    }

    pub(super) fn next(
        &mut self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<PiperSynthesisStep<'_>, PiperFailure> {
        if self.child.is_none() {
            return Err(PiperFailure::NoActiveSynthesis);
        }
        let maximum = usize::from(conduit_std_offers::PIPER_FRAMES_PER_BLOCK) * 2;
        loop {
            if cancelled() {
                return Err(PiperFailure::Cancelled);
            }
            if self
                .started
                .is_none_or(|started| started.elapsed() >= self.limits.timeout)
            {
                return Err(PiperFailure::Timeout);
            }
            if self.raw.len() >= maximum {
                return self.emit(maximum);
            }
            self.read_pcm_once()?;
            self.read_diagnostics_once()?;
            if self.raw.len() >= maximum {
                return self.emit(maximum);
            }
            if self.exit_success.is_none() {
                if let Some(status) = self
                    .child
                    .as_mut()
                    .expect("active session has a child")
                    .0
                    .try_wait()
                    .map_err(|_| PiperFailure::ProviderLost)?
                {
                    self.exit_success = Some(status.success());
                }
            }
            if self.exit_success == Some(false) {
                return Err(PiperFailure::ProviderLost);
            }
            if self.exit_success == Some(true) && self.stdout_closed {
                if !self.raw.is_empty() {
                    let remaining = self.raw.len();
                    return self.emit(remaining);
                }
                if self.frames == 0 {
                    return Err(PiperFailure::MalformedPcm);
                }
                if self.stderr_closed {
                    return Ok(PiperSynthesisStep::Complete(self.finish()));
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub(super) fn abort(&mut self) {
        self.child = None;
        self.stdout = None;
        self.stderr = None;
        self.started = None;
        self.raw.clear();
        self.encoded.clear();
    }

    fn read_pcm_once(&mut self) -> Result<(), PiperFailure> {
        if self.stdout_closed {
            return Ok(());
        }
        let mut chunk = [0_u8; READ_BYTES];
        match self
            .stdout
            .as_mut()
            .expect("active session has stdout")
            .read(&mut chunk)
        {
            Ok(0) => self.stdout_closed = true,
            Ok(read) => self.raw.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(_) => return Err(PiperFailure::ReadFailed),
        }
        Ok(())
    }

    fn read_diagnostics_once(&mut self) -> Result<(), PiperFailure> {
        if self.stderr_closed {
            return Ok(());
        }
        let mut chunk = [0_u8; 1_024];
        match self
            .stderr
            .as_mut()
            .expect("active session has stderr")
            .read(&mut chunk)
        {
            Ok(0) => self.stderr_closed = true,
            Ok(read) => {
                let remaining = MAXIMUM_DIAGNOSTIC_BYTES.saturating_sub(self.diagnostics.len());
                self.diagnostics
                    .extend_from_slice(&chunk[..read.min(remaining)]);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(_) => return Err(PiperFailure::ReadFailed),
        }
        Ok(())
    }

    fn emit(&mut self, bytes: usize) -> Result<PiperSynthesisStep<'_>, PiperFailure> {
        if bytes == 0 || !bytes.is_multiple_of(2) {
            return Err(PiperFailure::MalformedPcm);
        }
        let block_frames = u32::try_from(bytes / 2).map_err(|_| PiperFailure::OutputOverflow)?;
        let next_frames = self
            .frames
            .checked_add(block_frames)
            .filter(|value| *value <= self.limits.maximum_frames)
            .ok_or(PiperFailure::OutputOverflow)?;
        let next_blocks = self
            .blocks
            .checked_add(1)
            .filter(|value| *value <= self.limits.maximum_blocks)
            .ok_or(PiperFailure::BlockOverflow)?;
        let payload = &self.raw[..bytes];
        self.pcm_digest.update(payload);
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            self.sample_rate_hz,
            PcmChannelLayout::Mono,
            u16::try_from(block_frames).map_err(|_| PiperFailure::BlockOverflow)?,
            PIPER_CLOCK_ID,
            u64::from(self.frames),
            false,
        )
        .map_err(|_| PiperFailure::MalformedPcm)?;
        self.encoded.clear();
        self.encoded.extend_from_slice(&header.encode());
        self.encoded.extend_from_slice(payload);
        self.raw.copy_within(bytes.., 0);
        self.raw.truncate(self.raw.len() - bytes);
        self.frames = next_frames;
        self.blocks = next_blocks;
        Ok(PiperSynthesisStep::Block(&self.encoded))
    }

    fn finish(&mut self) -> PiperSynthesisReceipt {
        use core::fmt::Write as _;
        write!(self.pcm_sha256, "{:x}", self.pcm_digest.clone().finalize())
            .expect("writing a digest to a String is infallible");
        self.child = None;
        self.stdout = None;
        self.stderr = None;
        self.started = None;
        PiperSynthesisReceipt {
            text_sha256: core::mem::take(&mut self.text_sha256),
            pcm_sha256: core::mem::take(&mut self.pcm_sha256),
            frames: self.frames,
            blocks: self.blocks,
            diagnostic_bytes: self.diagnostics.len() as u16,
        }
    }
}

impl Drop for PiperSession {
    fn drop(&mut self) {
        self.raw.fill(0);
        self.encoded.fill(0);
    }
}

fn nonblocking(descriptor: i32) -> Result<(), PiperFailure> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
    {
        return Err(PiperFailure::InvalidProvider);
    }
    Ok(())
}

struct PiperChild(Child);

impl Drop for PiperChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
