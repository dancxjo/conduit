//! Exact local Piper discovery and bounded streaming synthesis.

use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, MAXIMUM_PCM_FRAMES_PER_BLOCK,
    PCM_FRAME_HEADER_ENCODED_LEN,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MAXIMUM_EXECUTABLE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_MODEL_BYTES: u64 = 256 * 1024 * 1024;
const MAXIMUM_CONFIG_BYTES: u64 = 64 * 1024;
const MAXIMUM_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const PIPER_CLOCK_ID: u64 = 0x5049_5045_5201;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiperLimits {
    pub maximum_text_bytes: u32,
    pub maximum_frames: u32,
    pub maximum_blocks: u16,
    pub timeout: Duration,
}

impl PiperLimits {
    pub fn validate(self) -> Result<Self, PiperFailure> {
        let block_frames = u32::from(MAXIMUM_PCM_FRAMES_PER_BLOCK);
        if self.maximum_text_bytes == 0
            || self.maximum_frames == 0
            || self.maximum_blocks == 0
            || self.timeout.is_zero()
            || self.maximum_frames.div_ceil(block_frames) > u32::from(self.maximum_blocks)
        {
            return Err(PiperFailure::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiperDiscovery {
    executable: PathBuf,
    model: PathBuf,
    config: PathBuf,
    library_path: Option<PathBuf>,
    pub executable_sha256: String,
    pub model_sha256: String,
    pub config_sha256: String,
    pub sample_rate_hz: u32,
    pub model_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiperFailure {
    MissingProvider,
    InvalidProvider,
    InvalidLimits,
    EmptyText,
    TextOverflow,
    SpawnFailed,
    WriteFailed,
    ReadFailed,
    MalformedPcm,
    OutputOverflow,
    BlockOverflow,
    Timeout,
    Cancelled,
    ProviderLost,
    ConsumerPressure,
}

impl core::fmt::Display for PiperFailure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "Piper provider refusal: {self:?}")
    }
}

impl std::error::Error for PiperFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiperSynthesisReceipt {
    pub text_sha256: String,
    pub pcm_sha256: String,
    pub frames: u32,
    pub blocks: u16,
    pub diagnostic_bytes: u16,
}

pub struct PiperSpeechAdapter {
    discovery: PiperDiscovery,
    limits: PiperLimits,
    raw: Vec<u8>,
    encoded: Vec<u8>,
    diagnostics: Vec<u8>,
}

impl PiperDiscovery {
    pub fn inspect(
        executable: impl AsRef<Path>,
        model: impl AsRef<Path>,
        config: impl AsRef<Path>,
        library_path: Option<PathBuf>,
    ) -> Result<Self, PiperFailure> {
        let executable = exact_file(executable.as_ref(), MAXIMUM_EXECUTABLE_BYTES)?;
        let model = exact_file(model.as_ref(), MAXIMUM_MODEL_BYTES)?;
        let config = exact_file(config.as_ref(), MAXIMUM_CONFIG_BYTES)?;
        if !is_executable(&executable)? {
            return Err(PiperFailure::InvalidProvider);
        }
        let config_bytes = std::fs::read(&config).map_err(|_| PiperFailure::InvalidProvider)?;
        let parsed: PiperConfig =
            serde_json::from_slice(&config_bytes).map_err(|_| PiperFailure::InvalidProvider)?;
        if !(8_000..=192_000).contains(&parsed.audio.sample_rate) {
            return Err(PiperFailure::InvalidProvider);
        }
        if library_path.as_ref().is_some_and(|path| !path.is_dir()) {
            return Err(PiperFailure::InvalidProvider);
        }
        let model_bytes = model
            .metadata()
            .map_err(|_| PiperFailure::InvalidProvider)?
            .len();
        Ok(Self {
            executable_sha256: digest_file(&executable)?,
            model_sha256: digest_file(&model)?,
            config_sha256: format!("{:x}", Sha256::digest(&config_bytes)),
            sample_rate_hz: parsed.audio.sample_rate,
            model_bytes,
            executable,
            model,
            config,
            library_path,
        })
    }

    pub fn initialize(self, limits: PiperLimits) -> Result<PiperSpeechAdapter, PiperFailure> {
        let limits = limits.validate()?;
        let block_bytes = usize::from(MAXIMUM_PCM_FRAMES_PER_BLOCK) * 2;
        Ok(PiperSpeechAdapter {
            discovery: self,
            limits,
            raw: Vec::with_capacity(block_bytes * 2),
            encoded: Vec::with_capacity(PCM_FRAME_HEADER_ENCODED_LEN + block_bytes),
            diagnostics: Vec::with_capacity(MAXIMUM_DIAGNOSTIC_BYTES),
        })
    }
}

impl PiperSpeechAdapter {
    pub fn discovery(&self) -> &PiperDiscovery {
        &self.discovery
    }

    pub fn limits(&self) -> PiperLimits {
        self.limits
    }

    pub fn synthesize(
        &mut self,
        text: &str,
        mut cancelled: impl FnMut() -> bool,
        mut consume: impl FnMut(&[u8]) -> Result<(), ()>,
    ) -> Result<PiperSynthesisReceipt, PiperFailure> {
        if text.is_empty() {
            return Err(PiperFailure::EmptyText);
        }
        if text.len() > self.limits.maximum_text_bytes as usize {
            return Err(PiperFailure::TextOverflow);
        }
        self.raw.clear();
        self.encoded.clear();
        self.diagnostics.clear();
        let mut child = PiperChild(self.spawn()?);
        let mut stdin = child.0.stdin.take().ok_or(PiperFailure::SpawnFailed)?;
        stdin
            .write_all(text.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .map_err(|_| PiperFailure::WriteFailed)?;
        drop(stdin);
        let mut stdout = child.0.stdout.take().ok_or(PiperFailure::SpawnFailed)?;
        let mut stderr = child.0.stderr.take().ok_or(PiperFailure::SpawnFailed)?;
        nonblocking(stdout.as_raw_fd())?;
        nonblocking(stderr.as_raw_fd())?;

        let started = Instant::now();
        let mut frames = 0_u32;
        let mut blocks = 0_u16;
        let mut pcm_digest = Sha256::new();
        let mut stdout_closed = false;
        let mut stderr_closed = false;
        loop {
            if cancelled() {
                return Err(PiperFailure::Cancelled);
            }
            if started.elapsed() >= self.limits.timeout {
                return Err(PiperFailure::Timeout);
            }
            stdout_closed |= self.read_pcm(
                &mut stdout,
                &mut frames,
                &mut blocks,
                &mut pcm_digest,
                &mut consume,
            )?;
            stderr_closed |= read_diagnostics(&mut stderr, &mut self.diagnostics)?;
            if let Some(status) = child.0.try_wait().map_err(|_| PiperFailure::ProviderLost)? {
                while !stdout_closed {
                    stdout_closed |= self.read_pcm(
                        &mut stdout,
                        &mut frames,
                        &mut blocks,
                        &mut pcm_digest,
                        &mut consume,
                    )?;
                }
                while !stderr_closed {
                    stderr_closed |= read_diagnostics(&mut stderr, &mut self.diagnostics)?;
                }
                if !status.success() {
                    return Err(PiperFailure::ProviderLost);
                }
                if !self.raw.is_empty() {
                    let remaining = self.raw.len();
                    self.emit(
                        remaining,
                        &mut frames,
                        &mut blocks,
                        &mut pcm_digest,
                        &mut consume,
                    )?;
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if !self.raw.is_empty() || frames == 0 {
            return Err(PiperFailure::MalformedPcm);
        }
        Ok(PiperSynthesisReceipt {
            text_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            pcm_sha256: format!("{:x}", pcm_digest.finalize()),
            frames,
            blocks,
            diagnostic_bytes: self.diagnostics.len() as u16,
        })
    }

    fn spawn(&self) -> Result<Child, PiperFailure> {
        let mut command = Command::new(&self.discovery.executable);
        command
            .arg("--model")
            .arg(&self.discovery.model)
            .arg("--config")
            .arg(&self.discovery.config)
            .arg("--output_raw")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = &self.discovery.library_path {
            command.env("LD_LIBRARY_PATH", path);
        }
        command.spawn().map_err(|_| PiperFailure::SpawnFailed)
    }

    fn read_pcm(
        &mut self,
        stdout: &mut impl Read,
        frames: &mut u32,
        blocks: &mut u16,
        digest: &mut Sha256,
        consume: &mut impl FnMut(&[u8]) -> Result<(), ()>,
    ) -> Result<bool, PiperFailure> {
        let maximum = usize::from(MAXIMUM_PCM_FRAMES_PER_BLOCK) * 2;
        let mut chunk = [0_u8; 4_096];
        match stdout.read(&mut chunk) {
            Ok(0) => return Ok(true),
            Ok(read) => self.raw.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(false),
            Err(_) => return Err(PiperFailure::ReadFailed),
        }
        while self.raw.len() >= maximum {
            self.emit(maximum, frames, blocks, digest, consume)?;
        }
        Ok(false)
    }

    fn emit(
        &mut self,
        bytes: usize,
        frames: &mut u32,
        blocks: &mut u16,
        digest: &mut Sha256,
        consume: &mut impl FnMut(&[u8]) -> Result<(), ()>,
    ) -> Result<(), PiperFailure> {
        if bytes == 0 || !bytes.is_multiple_of(2) {
            return Err(PiperFailure::MalformedPcm);
        }
        let block_frames = u32::try_from(bytes / 2).map_err(|_| PiperFailure::OutputOverflow)?;
        let next_frames = frames
            .checked_add(block_frames)
            .filter(|value| *value <= self.limits.maximum_frames)
            .ok_or(PiperFailure::OutputOverflow)?;
        let next_blocks = blocks
            .checked_add(1)
            .filter(|value| *value <= self.limits.maximum_blocks)
            .ok_or(PiperFailure::BlockOverflow)?;
        let payload = &self.raw[..bytes];
        digest.update(payload);
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            self.discovery.sample_rate_hz,
            PcmChannelLayout::Mono,
            u16::try_from(block_frames).map_err(|_| PiperFailure::BlockOverflow)?,
            PIPER_CLOCK_ID,
            u64::from(*frames),
            false,
        )
        .map_err(|_| PiperFailure::MalformedPcm)?;
        self.encoded.clear();
        self.encoded.extend_from_slice(&header.encode());
        self.encoded.extend_from_slice(payload);
        consume(&self.encoded).map_err(|_| PiperFailure::ConsumerPressure)?;
        self.raw.drain(..bytes);
        *frames = next_frames;
        *blocks = next_blocks;
        Ok(())
    }
}

impl Drop for PiperSpeechAdapter {
    fn drop(&mut self) {
        self.raw.fill(0);
        self.encoded.fill(0);
    }
}

#[derive(Deserialize)]
struct PiperConfig {
    audio: PiperAudioConfig,
}

#[derive(Deserialize)]
struct PiperAudioConfig {
    sample_rate: u32,
}

fn exact_file(path: &Path, maximum: u64) -> Result<PathBuf, PiperFailure> {
    let path = path
        .canonicalize()
        .map_err(|_| PiperFailure::MissingProvider)?;
    let metadata = path.metadata().map_err(|_| PiperFailure::MissingProvider)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(PiperFailure::InvalidProvider);
    }
    Ok(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool, PiperFailure> {
    use std::os::unix::fs::PermissionsExt;
    Ok(path
        .metadata()
        .map_err(|_| PiperFailure::InvalidProvider)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool, PiperFailure> {
    Ok(false)
}

fn digest_file(path: &Path) -> Result<String, PiperFailure> {
    let mut file = File::open(path).map_err(|_| PiperFailure::InvalidProvider)?;
    let mut digest = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut bytes)
            .map_err(|_| PiperFailure::InvalidProvider)?;
        if read == 0 {
            break;
        }
        digest.update(&bytes[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn nonblocking(descriptor: i32) -> Result<(), PiperFailure> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
    {
        return Err(PiperFailure::InvalidProvider);
    }
    Ok(())
}

fn read_diagnostics(
    stderr: &mut impl Read,
    diagnostics: &mut Vec<u8>,
) -> Result<bool, PiperFailure> {
    let mut chunk = [0_u8; 1_024];
    match stderr.read(&mut chunk) {
        Ok(0) => Ok(true),
        Ok(read) => {
            let remaining = MAXIMUM_DIAGNOSTIC_BYTES.saturating_sub(diagnostics.len());
            diagnostics.extend_from_slice(&chunk[..read.min(remaining)]);
            Ok(false)
        }
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(false),
        Err(_) => Err(PiperFailure::ReadFailed),
    }
}

struct PiperChild(Child);

impl Drop for PiperChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
#[path = "piper_tests.rs"]
mod tests;
