//! Exact local Piper discovery and bounded streaming synthesis.

use super::{session::PiperSession, PiperSynthesisStep};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const MAXIMUM_EXECUTABLE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_MODEL_BYTES: u64 = 256 * 1024 * 1024;
const MAXIMUM_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiperLimits {
    pub maximum_text_bytes: u32,
    pub maximum_frames: u32,
    pub maximum_blocks: u16,
    pub timeout: Duration,
}

impl PiperLimits {
    pub fn validate(self) -> Result<Self, PiperFailure> {
        let block_frames = u32::from(conduit_std_offers::PIPER_FRAMES_PER_BLOCK);
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
    InvalidText,
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
    ProviderBusy,
    NoActiveSynthesis,
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
    session: PiperSession,
    last_receipt: Option<PiperSynthesisReceipt>,
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
        let sample_rate_hz = self.sample_rate_hz;
        Ok(PiperSpeechAdapter {
            discovery: self,
            limits,
            session: PiperSession::new(limits, sample_rate_hz),
            last_receipt: None,
        })
    }
}

impl PiperSpeechAdapter {
    pub(crate) fn is_active(&self) -> bool {
        self.session.is_active()
    }

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
        self.begin(text)?;
        loop {
            match self.next(&mut cancelled) {
                Ok(PiperSynthesisStep::Block(block)) => {
                    if consume(block).is_err() {
                        self.abort();
                        return Err(PiperFailure::ConsumerPressure);
                    }
                }
                Ok(PiperSynthesisStep::Complete(receipt)) => return Ok(receipt),
                Err(failure) => {
                    self.abort();
                    return Err(failure);
                }
            }
        }
    }

    pub fn begin(&mut self, text: &str) -> Result<(), PiperFailure> {
        if self.session.is_active() {
            return Err(PiperFailure::ProviderBusy);
        }
        if text.is_empty() {
            return Err(PiperFailure::EmptyText);
        }
        if text.len() > self.limits.maximum_text_bytes as usize {
            return Err(PiperFailure::TextOverflow);
        }
        let child = self.spawn()?;
        self.session.begin(child, text)
    }

    pub fn next(
        &mut self,
        cancelled: impl FnMut() -> bool,
    ) -> Result<PiperSynthesisStep<'_>, PiperFailure> {
        let step = self.session.next(cancelled)?;
        if let PiperSynthesisStep::Complete(receipt) = &step {
            self.last_receipt = Some(receipt.clone());
        }
        Ok(step)
    }

    pub fn abort(&mut self) {
        self.session.abort();
    }

    pub(crate) fn take_receipt(&mut self) -> Option<PiperSynthesisReceipt> {
        self.last_receipt.take()
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

#[cfg(test)]
#[path = "piper_tests.rs"]
mod tests;
