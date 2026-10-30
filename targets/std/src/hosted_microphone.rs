//! Exact ALSA microphone discovery and bounded raw PCM capture.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub mod capture_flow;
mod flow_format;
mod flow_provider;
#[path = "hosted_microphone/provider_discovery.rs"]
mod provider_discovery;
mod provider_io;
pub use flow_provider::{MicrophoneFlow, MicrophoneFlowPoll, PreparedMicrophoneFlow};
use provider_discovery::capture_command;
#[path = "hosted_microphone/provider_identity.rs"]
mod provider_identity;
use provider_identity::{digest_file, exact_file};

pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const CHANNELS: u8 = 1;
pub const BYTES_PER_SAMPLE: u8 = 2;
pub const MAXIMUM_CAPTURE_MILLISECONDS: u32 = 6_000;
pub const MAXIMUM_RAW_PCM_BYTES: usize = SAMPLE_RATE_HZ as usize
    * CHANNELS as usize
    * BYTES_PER_SAMPLE as usize
    * MAXIMUM_CAPTURE_MILLISECONDS as usize
    / 1_000;
const MAXIMUM_EXECUTABLE_BYTES: u64 = 8 * 1024 * 1024;
const MAXIMUM_DIAGNOSTIC_BYTES: usize = 8 * 1024;
const MAXIMUM_DISCOVERY_BYTES: usize = 64 * 1024;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaMicrophoneObservation {
    pub card_index: u16,
    pub card_id: String,
    pub card_name: String,
    pub device: u16,
    pub device_name: String,
    pub base_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaMicrophoneDiscovery {
    executable: PathBuf,
    pub executable_sha256: String,
    pub observations: Vec<AlsaMicrophoneObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrophoneLimits {
    pub capture_milliseconds: u32,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MicrophoneFailure {
    MissingProvider = 1,
    InvalidProvider = 2,
    DiscoveryFailed = 3,
    InvalidDiscovery = 4,
    NoEndpoint = 5,
    SelectionDrift = 6,
    InvalidLimits = 7,
    SpawnFailed = 8,
    ProviderLost = 9,
    ReadFailed = 10,
    EmptyCapture = 11,
    ShortCapture = 12,
    OutputOverflow = 13,
    Timeout = 14,
    Cancelled = 15,
    InexactFormat = 16,
    ClockUnavailable = 17,
    PartialSampleFrame = 18,
    CaptureOverrun = 19,
    UnconfirmedFormat = 20,
}

impl core::fmt::Display for MicrophoneFailure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "ALSA microphone refusal: {self:?}")
    }
}

impl std::error::Error for MicrophoneFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrophoneCaptureReceipt {
    pub implementation_id: String,
    pub executable_sha256: String,
    pub base_identity: String,
    pub card_id: String,
    pub device: u16,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub bytes_per_sample: u8,
    pub capture_milliseconds: u32,
    pub raw_pcm_sha256: String,
    pub raw_pcm_bytes: u32,
    pub diagnostic_bytes: u16,
}

/// Provider format policy is selected by the Host, never inferred from failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaMicrophoneFormat {
    ExactHardware,
    AlsaPlug,
}
impl AlsaMicrophoneFormat {
    pub fn implementation_id(self) -> &'static str {
        match self {
            Self::ExactHardware => conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION,
            Self::AlsaPlug => conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION,
        }
    }
    fn device_prefix(self) -> &'static str {
        match self {
            Self::ExactHardware => "hw",
            Self::AlsaPlug => "plughw",
        }
    }
}

pub struct AlsaMicrophoneAdapter {
    format: AlsaMicrophoneFormat,
    discovery: AlsaMicrophoneDiscovery,
    selection: AlsaMicrophoneObservation,
    limits: MicrophoneLimits,
    last_receipt: Option<MicrophoneCaptureReceipt>,
}

impl AlsaMicrophoneAdapter {
    pub fn implementation_id(&self) -> &'static str {
        self.format.implementation_id()
    }
    pub fn offer(&self) -> conduit_core::CapabilityOffer {
        match self.format {
            AlsaMicrophoneFormat::ExactHardware => conduit_std_offers::microphone_clip_offer(),
            AlsaMicrophoneFormat::AlsaPlug => conduit_std_offers::converted_microphone_clip_offer(),
        }
    }

    pub fn flow_offer(&self) -> conduit_core::CapabilityOffer {
        conduit_std_offers::microphone_flow_offer(self.format == AlsaMicrophoneFormat::AlsaPlug)
    }

    pub fn executable_sha256(&self) -> &str {
        &self.discovery.executable_sha256
    }

    pub fn resource_pool_id(&self) -> conduit_core::ResourcePoolId {
        conduit_core::ResourcePoolId::from(format!(
            "std/audio/alsa-input/{}/card-{}/device-{}",
            self.selection.base_identity, self.selection.card_id, self.selection.device
        ))
    }

    pub fn observation(&self) -> &AlsaMicrophoneObservation {
        &self.selection
    }

    pub fn limits(&self) -> MicrophoneLimits {
        self.limits
    }

    pub fn capture(
        &mut self,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, MicrophoneFailure> {
        self.last_receipt = None;
        if cancelled() {
            return Err(MicrophoneFailure::Cancelled);
        }
        let started = Instant::now();
        let current = exact_file(&self.discovery.executable)?;
        if current != self.discovery.executable
            || digest_file(&current)? != self.discovery.executable_sha256
        {
            return Err(MicrophoneFailure::SelectionDrift);
        }
        if cancelled() {
            return Err(MicrophoneFailure::Cancelled);
        }
        if started.elapsed() >= self.limits.timeout {
            return Err(MicrophoneFailure::Timeout);
        }
        let mut child = self.spawn()?;
        let maximum_audio_bytes = raw_bytes_for_duration(self.limits.capture_milliseconds);
        let (status, audio, diagnostics) = provider_io::collect(
            &mut child,
            maximum_audio_bytes,
            MAXIMUM_DIAGNOSTIC_BYTES,
            started,
            self.limits.timeout,
            cancelled,
        )?;
        self.finish_capture(status.success(), audio, diagnostics, maximum_audio_bytes)
    }

    fn finish_capture(
        &mut self,
        provider_succeeded: bool,
        audio: BoundedRead,
        diagnostics: BoundedRead,
        maximum_audio_bytes: usize,
    ) -> Result<Vec<u8>, MicrophoneFailure> {
        if !provider_succeeded {
            return Err(MicrophoneFailure::ProviderLost);
        }
        if diagnostics.overflowed {
            return Err(MicrophoneFailure::OutputOverflow);
        }
        if diagnostics
            .bytes
            .windows(b"rate is not accurate".len())
            .any(|v| v == b"rate is not accurate")
        {
            return Err(MicrophoneFailure::InexactFormat);
        }
        if audio.overflowed {
            return Err(MicrophoneFailure::OutputOverflow);
        }
        if audio.bytes.is_empty() {
            return Err(MicrophoneFailure::EmptyCapture);
        }
        if audio.bytes.len() != maximum_audio_bytes {
            return Err(MicrophoneFailure::ShortCapture);
        }
        self.last_receipt = Some(MicrophoneCaptureReceipt {
            implementation_id: self.implementation_id().into(),
            executable_sha256: self.discovery.executable_sha256.clone(),
            base_identity: self.selection.base_identity.clone(),
            card_id: self.selection.card_id.clone(),
            device: self.selection.device,
            sample_rate_hz: SAMPLE_RATE_HZ,
            channels: CHANNELS,
            bytes_per_sample: BYTES_PER_SAMPLE,
            capture_milliseconds: self.limits.capture_milliseconds,
            raw_pcm_sha256: format!("{:x}", Sha256::digest(&audio.bytes)),
            raw_pcm_bytes: u32::try_from(audio.bytes.len())
                .map_err(|_| MicrophoneFailure::OutputOverflow)?,
            diagnostic_bytes: u16::try_from(diagnostics.bytes.len())
                .map_err(|_| MicrophoneFailure::OutputOverflow)?,
        });
        Ok(audio.bytes)
    }

    pub fn capture_clip(
        &mut self,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, MicrophoneFailure> {
        let raw = self.capture(cancelled)?;
        let bytes_per_frame = CHANNELS as usize * BYTES_PER_SAMPLE as usize;
        let block_bytes =
            usize::from(conduit_audio::MAXIMUM_PCM_FRAMES_PER_BLOCK) * bytes_per_frame;
        let mut encoded_frames = Vec::with_capacity(raw.len().div_ceil(block_bytes));
        let mut start_frame = 0_u64;
        for payload in raw.chunks(block_bytes) {
            let frame_count = u16::try_from(payload.len() / bytes_per_frame)
                .map_err(|_| MicrophoneFailure::OutputOverflow)?;
            let header = conduit_audio::PcmFrameHeader::new(
                conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
                SAMPLE_RATE_HZ,
                conduit_audio::PcmChannelLayout::Mono,
                frame_count,
                1,
                start_frame,
                false,
            )
            .map_err(|_| MicrophoneFailure::ReadFailed)?;
            encoded_frames.push(
                header
                    .encode_frame(payload)
                    .map_err(|_| MicrophoneFailure::ReadFailed)?,
            );
            start_frame = start_frame
                .checked_add(u64::from(frame_count))
                .ok_or(MicrophoneFailure::OutputOverflow)?;
        }
        let frames = encoded_frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
        conduit_audio::encode_pcm_clip(&frames).map_err(|_| MicrophoneFailure::OutputOverflow)
    }

    pub fn take_receipt(&mut self) -> Option<MicrophoneCaptureReceipt> {
        self.last_receipt.take()
    }

    fn spawn(&self) -> Result<Child, MicrophoneFailure> {
        self.capture_process()
            .spawn()
            .map_err(|_| MicrophoneFailure::SpawnFailed)
    }

    fn capture_process(&self) -> Command {
        self.capture_process_for(capture_frames(self.limits.capture_milliseconds))
    }
    fn capture_process_for(&self, frames: u32) -> Command {
        let device = format!(
            "{}:CARD={},DEV={}",
            self.format.device_prefix(),
            self.selection.card_id,
            self.selection.device
        );
        let samples = frames.to_string();
        let mut command = capture_command(&self.discovery.executable);
        command
            .args([
                "--device",
                &device,
                "--format",
                "S16_LE",
                "--rate",
                "16000",
                "--channels",
                "1",
                "--samples",
                &samples,
                "--file-type",
                "raw",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

struct BoundedRead {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn validate_limits(limits: MicrophoneLimits) -> Result<(), MicrophoneFailure> {
    if limits.capture_milliseconds == 0
        || limits.capture_milliseconds > MAXIMUM_CAPTURE_MILLISECONDS
        || limits.timeout.is_zero()
    {
        return Err(MicrophoneFailure::InvalidLimits);
    }
    Ok(())
}

fn capture_frames(milliseconds: u32) -> u32 {
    milliseconds * (SAMPLE_RATE_HZ / 1_000)
}

fn raw_bytes_for_duration(milliseconds: u32) -> usize {
    capture_frames(milliseconds) as usize * CHANNELS as usize * BYTES_PER_SAMPLE as usize
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_arecord_list(
    listing: &str,
    sound_class: &Path,
) -> Result<Vec<AlsaMicrophoneObservation>, MicrophoneFailure> {
    let mut observations = Vec::new();
    for line in listing.lines().filter(|line| line.starts_with("card ")) {
        let (card_part, device_part) = line
            .split_once(", device ")
            .ok_or(MicrophoneFailure::InvalidDiscovery)?;
        let card_part = card_part
            .strip_prefix("card ")
            .ok_or(MicrophoneFailure::InvalidDiscovery)?;
        let (card_index, card_tail) = card_part
            .split_once(": ")
            .ok_or(MicrophoneFailure::InvalidDiscovery)?;
        let (card_id, card_name) = bracketed_name(card_tail)?;
        let (device, device_tail) = device_part
            .split_once(": ")
            .ok_or(MicrophoneFailure::InvalidDiscovery)?;
        let (_, device_name) = bracketed_name(device_tail)?;
        let card_index = card_index
            .parse::<u16>()
            .map_err(|_| MicrophoneFailure::InvalidDiscovery)?;
        let device = device
            .parse::<u16>()
            .map_err(|_| MicrophoneFailure::InvalidDiscovery)?;
        observations.push(AlsaMicrophoneObservation {
            card_index,
            card_id: card_id.to_owned(),
            card_name: card_name.to_owned(),
            device,
            device_name: device_name.to_owned(),
            base_identity: base_identity(sound_class, card_index),
        });
    }
    observations.sort_by(|left, right| {
        (&left.base_identity, &left.card_id, left.device).cmp(&(
            &right.base_identity,
            &right.card_id,
            right.device,
        ))
    });
    Ok(observations)
}

fn bracketed_name(value: &str) -> Result<(&str, &str), MicrophoneFailure> {
    let open = value.find('[').ok_or(MicrophoneFailure::InvalidDiscovery)?;
    let close = value[open + 1..]
        .find(']')
        .map(|index| open + 1 + index)
        .ok_or(MicrophoneFailure::InvalidDiscovery)?;
    Ok((value[..open].trim(), value[open + 1..close].trim()))
}

fn base_identity(sound_class: &Path, card_index: u16) -> String {
    let device = sound_class.join(format!("card{card_index}/device"));
    std::fs::canonicalize(device)
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_owned()))
        .and_then(|name| name.to_str().map(str::to_owned))
        .unwrap_or_else(|| format!("alsa-card-{card_index}"))
}

#[path = "hosted_microphone_host.rs"]
mod host;

#[cfg(test)]
#[path = "hosted_microphone_tests.rs"]
mod tests;
