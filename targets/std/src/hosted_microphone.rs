//! Exact ALSA microphone discovery and bounded raw PCM capture.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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
}

impl core::fmt::Display for MicrophoneFailure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "ALSA microphone refusal: {self:?}")
    }
}

impl std::error::Error for MicrophoneFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrophoneCaptureReceipt {
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

pub struct AlsaMicrophoneAdapter {
    discovery: AlsaMicrophoneDiscovery,
    selection: AlsaMicrophoneObservation,
    limits: MicrophoneLimits,
    last_receipt: Option<MicrophoneCaptureReceipt>,
}

impl AlsaMicrophoneDiscovery {
    /// Enumerates ALSA control metadata without opening a PCM capture stream.
    pub fn inspect(executable: impl AsRef<Path>) -> Result<Self, MicrophoneFailure> {
        let executable = exact_file(executable.as_ref())?;
        let output = bounded_discovery(&executable)?;
        let listing =
            std::str::from_utf8(&output).map_err(|_| MicrophoneFailure::InvalidDiscovery)?;
        let observations = parse_arecord_list(listing, Path::new("/sys/class/sound"))?;
        Ok(Self {
            executable_sha256: digest_file(&executable)?,
            executable,
            observations,
        })
    }

    pub fn initialize(
        self,
        selected: &AlsaMicrophoneObservation,
        limits: MicrophoneLimits,
    ) -> Result<AlsaMicrophoneAdapter, MicrophoneFailure> {
        validate_limits(limits)?;
        if self.observations.is_empty() {
            return Err(MicrophoneFailure::NoEndpoint);
        }
        let selection = self
            .observations
            .iter()
            .find(|candidate| *candidate == selected)
            .cloned()
            .ok_or(MicrophoneFailure::SelectionDrift)?;
        Ok(AlsaMicrophoneAdapter {
            discovery: self,
            selection,
            limits,
            last_receipt: None,
        })
    }
}

impl AlsaMicrophoneAdapter {
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
        let mut child = self.spawn()?;
        let stdout = child.stdout.take().ok_or(MicrophoneFailure::SpawnFailed)?;
        let stderr = child.stderr.take().ok_or(MicrophoneFailure::SpawnFailed)?;
        let maximum_audio_bytes = raw_bytes_for_duration(self.limits.capture_milliseconds);
        let audio = std::thread::spawn(move || bounded_read(stdout, maximum_audio_bytes));
        let diagnostics =
            std::thread::spawn(move || bounded_read(stderr, MAXIMUM_DIAGNOSTIC_BYTES));
        let started = Instant::now();
        let status = loop {
            if cancelled() {
                stop(&mut child);
                let _ = audio.join();
                let _ = diagnostics.join();
                return Err(MicrophoneFailure::Cancelled);
            }
            if started.elapsed() >= self.limits.timeout {
                stop(&mut child);
                let _ = audio.join();
                let _ = diagnostics.join();
                return Err(MicrophoneFailure::Timeout);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(_) => {
                    stop(&mut child);
                    let _ = audio.join();
                    let _ = diagnostics.join();
                    return Err(MicrophoneFailure::ProviderLost);
                }
            }
        };
        let audio = audio.join().map_err(|_| MicrophoneFailure::ReadFailed)??;
        let diagnostics = diagnostics
            .join()
            .map_err(|_| MicrophoneFailure::ReadFailed)??;
        if !status.success() {
            return Err(MicrophoneFailure::ProviderLost);
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
        let mut encoded_frames = Vec::new();
        let mut start_frame = 0_u64;
        for payload in raw.chunks(SAMPLE_RATE_HZ as usize * bytes_per_frame) {
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
        let device = format!(
            "hw:CARD={},DEV={}",
            self.selection.card_id, self.selection.device
        );
        let samples = capture_frames(self.limits.capture_milliseconds).to_string();
        Command::new(&self.discovery.executable)
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
                "--type",
                "raw",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| MicrophoneFailure::SpawnFailed)
    }
}

struct BoundedRead {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn bounded_read(mut reader: impl Read, maximum: usize) -> Result<BoundedRead, MicrophoneFailure> {
    let mut bytes = Vec::with_capacity(maximum);
    let mut overflowed = false;
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| MicrophoneFailure::ReadFailed)?;
        if count == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        overflowed |= count > remaining;
    }
    Ok(BoundedRead { bytes, overflowed })
}

fn bounded_discovery(executable: &Path) -> Result<Vec<u8>, MicrophoneFailure> {
    let mut child = Command::new(executable)
        .arg("-l")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(MicrophoneFailure::DiscoveryFailed)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(MicrophoneFailure::DiscoveryFailed)?;
    let listing = std::thread::spawn(move || bounded_read(stdout, MAXIMUM_DISCOVERY_BYTES));
    let diagnostics = std::thread::spawn(move || bounded_read(stderr, MAXIMUM_DIAGNOSTIC_BYTES));
    let started = Instant::now();
    let status = loop {
        if started.elapsed() >= DISCOVERY_TIMEOUT {
            stop(&mut child);
            let _ = listing.join();
            let _ = diagnostics.join();
            return Err(MicrophoneFailure::DiscoveryFailed);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => {
                stop(&mut child);
                return Err(MicrophoneFailure::DiscoveryFailed);
            }
        }
    };
    let listing = listing
        .join()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    let diagnostics = diagnostics
        .join()
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?
        .map_err(|_| MicrophoneFailure::DiscoveryFailed)?;
    if !status.success() || listing.overflowed || diagnostics.overflowed {
        return Err(MicrophoneFailure::DiscoveryFailed);
    }
    Ok(listing.bytes)
}

fn exact_file(path: &Path) -> Result<PathBuf, MicrophoneFailure> {
    let path = path
        .canonicalize()
        .map_err(|_| MicrophoneFailure::MissingProvider)?;
    let metadata = path
        .metadata()
        .map_err(|_| MicrophoneFailure::MissingProvider)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_EXECUTABLE_BYTES {
        return Err(MicrophoneFailure::InvalidProvider);
    }
    if !is_executable(&path)? {
        return Err(MicrophoneFailure::InvalidProvider);
    }
    Ok(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool, MicrophoneFailure> {
    use std::os::unix::fs::PermissionsExt;
    Ok(path
        .metadata()
        .map_err(|_| MicrophoneFailure::InvalidProvider)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool, MicrophoneFailure> {
    Ok(false)
}

fn digest_file(path: &Path) -> Result<String, MicrophoneFailure> {
    let mut file = std::fs::File::open(path).map_err(|_| MicrophoneFailure::InvalidProvider)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| MicrophoneFailure::InvalidProvider)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
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
