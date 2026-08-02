//! Opt-in hosted audio implementations.
//!
//! Semantic audio frames and capture/playback node contracts remain in
//! `conduit-media`. This crate owns host-side adapters such as the
//! closed-inventory ALSA boundary and supervised FFmpeg/SoX transforms without
//! exposing their names, commands, or callback types through media ports.

pub mod transform_implementations;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::thread::{self, JoinHandle};

use conduit_compile::{
    HostServiceAuthorityObservationInput, ObservedHostServiceAuthority,
    observed_host_service_authority, observed_host_service_constraints,
};
use conduit_core::{Id, SemanticHash, StopPolicy};
use conduit_media::{
    AUDIO_CAPTURE_CONTRACT, AUDIO_FRAME_TYPE, AUDIO_PLAYBACK_CONTRACT, ChannelLayout, PcmChunk,
    decode_pcm_chunk, encode_pcm_chunk,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, ExactHostedServiceBinding, Handler, HostedServiceCleanup,
    HostedServiceInterest, HostedServiceStep, HostedServiceStepContext, Registry, RegistryError,
    ResolutionError, RunIo, RuntimeError, Value,
};
use sha2::{Digest, Sha256};

pub const DEFAULT_ARECORD_PATH: &str = "/usr/bin/arecord";
pub const DEFAULT_APLAY_PATH: &str = "/usr/bin/aplay";
pub const MAXIMUM_OBSERVATION_BYTES: usize = 64 * 1024;
pub const MAXIMUM_TOOL_BYTES: u64 = 16 * 1024 * 1024;
pub const MAXIMUM_OBSERVED_DEVICES: usize = 64;
pub const ALSA_CAPTURE_IMPLEMENTATION_ID: &str = "conduit.audio/capture-alsa-hosted";
pub const ALSA_PLAYBACK_IMPLEMENTATION_ID: &str = "conduit.audio/playback-alsa-hosted";

const CAPTURE_AUTHORITY: SemanticHash = SemanticHash::from_bytes([0x65; 32]);
const PLAYBACK_AUTHORITY: SemanticHash = SemanticHash::from_bytes([0x66; 32]);
const CAPTURE_GRANT: &str = "conduit.audio/grant/alsa-capture";
const PLAYBACK_GRANT: &str = "conduit.audio/grant/alsa-playback";
const CAPTURE_RESOURCE_PREFIX: &str = "conduit.audio/device/alsa/capture/";
const PLAYBACK_RESOURCE_PREFIX: &str = "conduit.audio/device/alsa/playback/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceDirection {
    Capture,
    Playback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAudioDevice {
    pub direction: DeviceDirection,
    pub resource_id: String,
    pub backend_name: String,
    pub friendly_label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationState {
    Available,
    Unsupported,
    Unavailable,
    PermissionDenied,
    Overflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlsaObservation {
    pub observation_id: String,
    pub generation: u64,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub backend_identity: String,
    pub arecord_digest: String,
    pub aplay_digest: String,
    pub devices: Vec<ObservedAudioDevice>,
    pub arecord_path: PathBuf,
    pub aplay_path: PathBuf,
}

impl AlsaObservation {
    #[must_use]
    pub fn is_fresh_at(&self, tick: u64) -> bool {
        self.observed_at_tick <= tick && tick <= self.valid_until_tick
    }

    #[must_use]
    pub fn device(
        &self,
        resource_id: &str,
        direction: DeviceDirection,
    ) -> Option<&ObservedAudioDevice> {
        self.devices
            .iter()
            .find(|device| device.resource_id == resource_id && device.direction == direction)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlsaObservationReport {
    pub state: ObservationState,
    pub reason_code: &'static str,
    pub detail: String,
    pub compiled_support: bool,
    pub observation: Option<AlsaObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlsaToolchain {
    pub arecord_path: PathBuf,
    pub aplay_path: PathBuf,
}

impl Default for AlsaToolchain {
    fn default() -> Self {
        Self {
            arecord_path: PathBuf::from(DEFAULT_ARECORD_PATH),
            aplay_path: PathBuf::from(DEFAULT_APLAY_PATH),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryPhase {
    Described,
    Resolved,
    Open,
    Started,
    FirstSample,
    Draining,
    Stopped,
    Closed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryEvent {
    DescribedWithoutOpen,
    ResolvedWithoutOpen,
    Opened,
    Started,
    Waiting,
    FirstSample,
    PlaybackCommitted,
    Underrun,
    Overrun,
    Discontinuity,
    ClockDrift,
    ProviderLost,
    CancellationBeforeOpen,
    CancellationAfterOpen,
    CancellationRunning,
    CancellationDuringDrain,
    DrainStarted,
    Drained,
    Stopped,
    Closed,
    CleanupTimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioBoundaryLifecycle {
    phase: BoundaryPhase,
    events: Vec<BoundaryEvent>,
    maximum_events: usize,
}

impl AudioBoundaryLifecycle {
    #[must_use]
    pub fn described(maximum_events: usize) -> Self {
        let mut lifecycle = Self {
            phase: BoundaryPhase::Described,
            events: Vec::with_capacity(maximum_events),
            maximum_events,
        };
        lifecycle.record(BoundaryEvent::DescribedWithoutOpen);
        lifecycle
    }

    pub fn resolved(&mut self) {
        self.phase = BoundaryPhase::Resolved;
        self.record(BoundaryEvent::ResolvedWithoutOpen);
    }

    pub fn opened(&mut self) {
        self.phase = BoundaryPhase::Open;
        self.record(BoundaryEvent::Opened);
    }

    pub fn started(&mut self) {
        self.phase = BoundaryPhase::Started;
        self.record(BoundaryEvent::Started);
    }

    pub fn waiting(&mut self) {
        self.record(BoundaryEvent::Waiting);
    }

    pub fn first_sample(&mut self) {
        self.phase = BoundaryPhase::FirstSample;
        self.record(BoundaryEvent::FirstSample);
    }

    pub fn playback_committed(&mut self) {
        self.record(BoundaryEvent::PlaybackCommitted);
    }

    pub fn drain(&mut self) {
        self.phase = BoundaryPhase::Draining;
        self.record(BoundaryEvent::DrainStarted);
    }

    pub fn drained(&mut self) {
        self.record(BoundaryEvent::Drained);
    }

    pub fn stop(&mut self) {
        self.phase = BoundaryPhase::Stopped;
        self.record(BoundaryEvent::Stopped);
    }

    pub fn close(&mut self) {
        self.phase = BoundaryPhase::Closed;
        self.record(BoundaryEvent::Closed);
    }

    pub fn fail(&mut self, event: BoundaryEvent) {
        self.phase = BoundaryPhase::Failed;
        self.record(event);
    }

    pub fn cancel(&mut self) {
        let event = match self.phase {
            BoundaryPhase::Described | BoundaryPhase::Resolved => {
                BoundaryEvent::CancellationBeforeOpen
            }
            BoundaryPhase::Open => BoundaryEvent::CancellationAfterOpen,
            BoundaryPhase::Draining => BoundaryEvent::CancellationDuringDrain,
            _ => BoundaryEvent::CancellationRunning,
        };
        self.record(event);
    }

    #[must_use]
    pub const fn phase(&self) -> BoundaryPhase {
        self.phase
    }

    #[must_use]
    pub fn events(&self) -> &[BoundaryEvent] {
        &self.events
    }

    fn record(&mut self, event: BoundaryEvent) {
        if self.events.len() < self.maximum_events {
            self.events.push(event);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioDeviceFixture {
    NoDevice,
    PermissionDenied,
    StaleObservation,
    FormatMismatch,
    DeviceBusy,
    ExclusiveConflict,
    Underrun,
    Overrun,
    ClockDrift,
    HotUnplug,
    ProviderRestart,
    CancellationBeforeOpen,
    CancellationAfterOpen,
    CancellationDuringDrain,
    CleanupTimeout,
    VirtualLoopbackEquivalence,
}

impl AudioDeviceFixture {
    pub const ALL: [Self; 16] = [
        Self::NoDevice,
        Self::PermissionDenied,
        Self::StaleObservation,
        Self::FormatMismatch,
        Self::DeviceBusy,
        Self::ExclusiveConflict,
        Self::Underrun,
        Self::Overrun,
        Self::ClockDrift,
        Self::HotUnplug,
        Self::ProviderRestart,
        Self::CancellationBeforeOpen,
        Self::CancellationAfterOpen,
        Self::CancellationDuringDrain,
        Self::CleanupTimeout,
        Self::VirtualLoopbackEquivalence,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoDevice => "no-device",
            Self::PermissionDenied => "permission-denied",
            Self::StaleObservation => "stale-observation",
            Self::FormatMismatch => "format-mismatch",
            Self::DeviceBusy => "device-busy",
            Self::ExclusiveConflict => "exclusive-conflict",
            Self::Underrun => "underrun",
            Self::Overrun => "overrun",
            Self::ClockDrift => "clock-drift",
            Self::HotUnplug => "hot-unplug",
            Self::ProviderRestart => "provider-restart",
            Self::CancellationBeforeOpen => "cancellation-before-open",
            Self::CancellationAfterOpen => "cancellation-after-open",
            Self::CancellationDuringDrain => "cancellation-during-drain",
            Self::CleanupTimeout => "cleanup-timeout",
            Self::VirtualLoopbackEquivalence => "virtual-loopback-equivalence",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureOutcome {
    pub accepted: bool,
    pub reason_code: &'static str,
    pub terminal: &'static str,
}

#[must_use]
pub const fn fixture_outcome(fixture: AudioDeviceFixture) -> FixtureOutcome {
    match fixture {
        AudioDeviceFixture::NoDevice => rejected("CND-AUDDEV-001", "unsupported"),
        AudioDeviceFixture::PermissionDenied => rejected("CND-AUDDEV-002", "permission-denied"),
        AudioDeviceFixture::StaleObservation => rejected("CND-AUDDEV-003", "stale-observation"),
        AudioDeviceFixture::FormatMismatch => rejected("CND-AUDDEV-004", "format-mismatch"),
        AudioDeviceFixture::DeviceBusy => rejected("CND-AUDDEV-005", "device-busy"),
        AudioDeviceFixture::ExclusiveConflict => rejected("CND-AUDDEV-006", "exclusive-conflict"),
        AudioDeviceFixture::Underrun => rejected("CND-AUDDEV-007", "underrun"),
        AudioDeviceFixture::Overrun => rejected("CND-AUDDEV-008", "overrun"),
        AudioDeviceFixture::ClockDrift => rejected("CND-AUDDEV-009", "clock-drift"),
        AudioDeviceFixture::HotUnplug => rejected("CND-AUDDEV-010", "hot-unplug"),
        AudioDeviceFixture::ProviderRestart => rejected("CND-AUDDEV-011", "provider-restart"),
        AudioDeviceFixture::CancellationBeforeOpen => {
            rejected("CND-AUDDEV-012", "cancelled-before-open")
        }
        AudioDeviceFixture::CancellationAfterOpen => {
            rejected("CND-AUDDEV-013", "cancelled-after-open")
        }
        AudioDeviceFixture::CancellationDuringDrain => {
            rejected("CND-AUDDEV-014", "cancelled-during-drain")
        }
        AudioDeviceFixture::CleanupTimeout => rejected("CND-AUDDEV-015", "cleanup-timeout"),
        AudioDeviceFixture::VirtualLoopbackEquivalence => FixtureOutcome {
            accepted: true,
            reason_code: "CND-AUDDEV-OK",
            terminal: "explicit-stop",
        },
    }
}

const fn rejected(reason_code: &'static str, terminal: &'static str) -> FixtureOutcome {
    FixtureOutcome {
        accepted: false,
        reason_code,
        terminal,
    }
}

pub fn observe_alsa_devices(
    toolchain: &AlsaToolchain,
    generation: u64,
    observed_at_tick: u64,
    validity_ticks: u64,
) -> AlsaObservationReport {
    let valid_until_tick = observed_at_tick.saturating_add(validity_ticks);
    let arecord_digest = match digest_file(&toolchain.arecord_path) {
        Ok(digest) => digest,
        Err(error) => return observation_error(error, "capture executable observation failed"),
    };
    let aplay_digest = match digest_file(&toolchain.aplay_path) {
        Ok(digest) => digest,
        Err(error) => return observation_error(error, "playback executable observation failed"),
    };
    let capture_inventory = match bounded_command(&toolchain.arecord_path, &["-L"]) {
        Ok(output) => output,
        Err(error) => return observation_error(error, "capture inventory observation failed"),
    };
    let playback_inventory = match bounded_command(&toolchain.aplay_path, &["-L"]) {
        Ok(output) => output,
        Err(error) => return observation_error(error, "playback inventory observation failed"),
    };
    let mut devices = parse_devices(&capture_inventory, DeviceDirection::Capture);
    devices.extend(parse_devices(
        &playback_inventory,
        DeviceDirection::Playback,
    ));
    devices.truncate(MAXIMUM_OBSERVED_DEVICES);
    if devices.is_empty() {
        return AlsaObservationReport {
            state: ObservationState::Unsupported,
            reason_code: "CND-AUDDEV-001",
            detail: "the observed ALSA inventory contains no exact hw or null device".to_owned(),
            compiled_support: true,
            observation: None,
        };
    }
    let backend_identity = hash_fields(&[
        b"conduit.audio/alsa-backend".as_slice(),
        arecord_digest.as_bytes(),
        aplay_digest.as_bytes(),
    ]);
    let mut observation_hasher = Sha256::new();
    observation_hasher.update(b"conduit.audio/alsa-observation\0");
    observation_hasher.update(backend_identity.as_bytes());
    for device in &devices {
        observation_hasher.update(device.resource_id.as_bytes());
        observation_hasher.update([match device.direction {
            DeviceDirection::Capture => 0,
            DeviceDirection::Playback => 1,
        }]);
    }
    let observation_id = format!("sha256:{:x}", observation_hasher.finalize());
    AlsaObservationReport {
        state: ObservationState::Available,
        reason_code: "CND-AUDDEV-OK",
        detail: "provider and finite exact device inventory observed without opening a device"
            .to_owned(),
        compiled_support: true,
        observation: Some(AlsaObservation {
            observation_id,
            generation,
            observed_at_tick,
            valid_until_tick,
            backend_identity,
            arecord_digest,
            aplay_digest,
            devices,
            arecord_path: toolchain.arecord_path.clone(),
            aplay_path: toolchain.aplay_path.clone(),
        }),
    }
}

fn observation_error(error: io::Error, detail: &str) -> AlsaObservationReport {
    let state = match error.kind() {
        io::ErrorKind::NotFound => ObservationState::Unavailable,
        io::ErrorKind::PermissionDenied => ObservationState::PermissionDenied,
        io::ErrorKind::OutOfMemory | io::ErrorKind::FileTooLarge => ObservationState::Overflow,
        _ => ObservationState::Unavailable,
    };
    let reason_code = match state {
        ObservationState::PermissionDenied => "CND-AUDDEV-002",
        ObservationState::Overflow => "CND-AUDDEV-016",
        _ => "CND-AUDDEV-001",
    };
    AlsaObservationReport {
        state,
        reason_code,
        detail: format!("{detail}: {error}"),
        compiled_support: true,
        observation: None,
    }
}

fn digest_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAXIMUM_TOOL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "audio backend executable exceeds the observation byte ceiling",
        ));
    }
    let mut hasher = Sha256::new();
    let mut remaining = metadata.len();
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let count = file.read(&mut buffer[..limit])?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn bounded_command(path: &Path, arguments: &[&str]) -> io::Result<Vec<u8>> {
    let mut child = Command::new(path)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("audio observation stdout pipe is absent"))?;
    let mut output = Vec::new();
    stdout
        .by_ref()
        .take((MAXIMUM_OBSERVATION_BYTES + 1) as u64)
        .read_to_end(&mut output)?;
    if output.len() > MAXIMUM_OBSERVATION_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "audio observation exceeded its byte ceiling",
        ));
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "audio observation command exited with {status}"
        )));
    }
    Ok(output)
}

fn parse_devices(output: &[u8], direction: DeviceDirection) -> Vec<ObservedAudioDevice> {
    let text = String::from_utf8_lossy(output);
    let lines = text.lines().collect::<Vec<_>>();
    let mut devices = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() || line.starts_with(char::is_whitespace) {
            continue;
        }
        let backend_name = line.trim();
        if backend_name != "null" && !backend_name.starts_with("hw:") {
            continue;
        }
        let friendly_label = lines
            .get(index + 1)
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .unwrap_or(backend_name)
            .to_owned();
        devices.push(ObservedAudioDevice {
            direction,
            resource_id: format!(
                "{}{backend_name}",
                match direction {
                    DeviceDirection::Capture => CAPTURE_RESOURCE_PREFIX,
                    DeviceDirection::Playback => PLAYBACK_RESOURCE_PREFIX,
                }
            ),
            backend_name: backend_name.to_owned(),
            friendly_label,
        });
    }
    devices
}

fn hash_fields(fields: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for field in fields {
        hasher.update((field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn resolution_error(detail: impl Into<String>) -> ResolutionError {
    ResolutionError::new("CND-AUDDEV-004", detail)
}

fn runtime_error(code: &'static str, detail: impl Into<String>) -> RuntimeError {
    RuntimeError::new(code, detail)
}

fn text<'a>(node: &'a Node, key: &str) -> Result<&'a str, ResolutionError> {
    node.config(key)
        .ok_or_else(|| resolution_error(format!("audio device field `{key}` must be public text")))
}

fn secret<'a>(node: &'a Node, key: &str) -> Result<&'a str, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::SecretReference(value)) => Ok(value),
        _ => Err(resolution_error(format!(
            "audio device field `{key}` must be an exact protected reference"
        ))),
    }
}

fn integer(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value)
            .map_err(|_| resolution_error(format!("audio device field `{key}` is negative"))),
        _ => Err(resolution_error(format!(
            "audio device field `{key}` must be an integer"
        ))),
    }
}

fn validate_profile(node: &Node, direction: DeviceDirection) -> Result<(), ResolutionError> {
    let contract = match direction {
        DeviceDirection::Capture => &AUDIO_CAPTURE_CONTRACT,
        DeviceDirection::Playback => &AUDIO_PLAYBACK_CONTRACT,
    };
    if node.config.len() != contract.config.fields.len() {
        return Err(resolution_error(
            "audio device node does not use the one current exact configuration shape",
        ));
    }
    let resource = secret(node, "device_resource")?;
    let resource_prefix = match direction {
        DeviceDirection::Capture => CAPTURE_RESOURCE_PREFIX,
        DeviceDirection::Playback => PLAYBACK_RESOURCE_PREFIX,
    };
    let backend_name = resource.strip_prefix(resource_prefix).ok_or_else(|| {
        resolution_error("audio device resource is not an observed ALSA identity")
    })?;
    if backend_name != "null" && !backend_name.starts_with("hw:") {
        return Err(resolution_error(
            "audio device resource must name an exact hw or null ALSA endpoint",
        ));
    }
    let grant = secret(node, "device_grant")?;
    let expected_grant = match direction {
        DeviceDirection::Capture => CAPTURE_GRANT,
        DeviceDirection::Playback => PLAYBACK_GRANT,
    };
    if grant != expected_grant
        || !text(node, "provider_observation")?.starts_with("sha256:")
        || !text(node, "backend_identity")?.starts_with("sha256:")
        || text(node, "sample_format")? != "pcm-s16le-interleaved"
        || integer(node, "sample_rate_hz")? != 48_000
        || text(node, "layout")? != "stereo-lr"
        || text(node, "sample_clock")? != "conduit.clock/alsa-device-48000"
        || text(node, "clock_correlation")? != "observed-monotonic-uncertain"
        || integer(node, "requested_period_frames")? != 32
        || integer(node, "admitted_period_frames")? != 32
        || integer(node, "requested_buffer_frames")? != 64
        || integer(node, "admitted_buffer_frames")? != 64
        || integer(node, "requested_latency_frames")? != 64
        || integer(node, "admitted_latency_frames")? != 64
        || text(node, "latency_classification")? != "observed"
        || !matches!(text(node, "sharing_mode")?, "shared-bounded" | "exclusive")
        || integer(node, "maximum_concurrent_streams")? != 1
        || text(node, "workload_class")? != "host-observed-best-effort"
        || text(node, "lifecycle")? != "standing"
        || text(node, "underrun")? != "wait-evidenced"
        || text(node, "overrun")? != "fail-terminal-evidenced"
        || text(node, "drift")? != "reject-evidenced"
        || text(node, "discontinuity")? != "fail-terminal-evidenced"
        || text(node, "provider_loss")? != "fail-terminal-evidenced"
        || text(node, "cancellation")? != "before-open-after-open-running-drain-distinct"
        || text(node, "drain")? != "flush-bounded"
        || text(node, "sensitivity")? != "restricted-audio"
        || integer(node, "maximum_frames_per_step")? != 32
        || integer(node, "maximum_host_queue_frames")? != 64
        || integer(node, "maximum_work")? > 256
        || integer(node, "maximum_evidence_events")? > 64
        || integer(node, "observation_generation")? == 0
        || integer(node, "observation_valid_until_tick")? < 12
        || integer(node, "lease_ticks")? == 0
        || integer(node, "revocation_grace_ticks")? == 0
        || integer(node, "cleanup_ticks")? == 0
    {
        return Err(resolution_error(
            "audio device request does not match the exact hosted ALSA profile",
        ));
    }
    let expected_commit = match direction {
        DeviceDirection::Capture => "first-sample-delivered",
        DeviceDirection::Playback => "backend-write-accepted",
    };
    if text(node, "commit_point")? != expected_commit {
        return Err(resolution_error(
            "audio device commit point does not match its direction",
        ));
    }
    Ok(())
}

fn validate_capture(node: &Node) -> Result<(), ResolutionError> {
    validate_profile(node, DeviceDirection::Capture)
}

fn validate_playback(node: &Node) -> Result<(), ResolutionError> {
    validate_profile(node, DeviceDirection::Playback)
}

fn validate_binding(
    node: &Node,
    binding: &ExactHostedServiceBinding,
    direction: DeviceDirection,
) -> Result<(), RuntimeError> {
    let resource = secret(node, "device_resource")
        .map_err(|error| runtime_error(error.code, error.message))?;
    let grant =
        secret(node, "device_grant").map_err(|error| runtime_error(error.code, error.message))?;
    let Some(authority) = binding.authorities.iter().find(|authority| {
        authority.resource_id == resource && authority.grant_id == grant && authority.check_at_use
    }) else {
        return Err(runtime_error(
            "CND-AUDDEV-017",
            "audio device use lacks the exact plan resource, grant, and use-time check",
        ));
    };
    if authority.valid_until_tick < binding.use_time_tick {
        return Err(runtime_error(
            "CND-AUDDEV-003",
            "audio device lease or grant is stale at use time",
        ));
    }
    let expected = match direction {
        DeviceDirection::Capture => ALSA_CAPTURE_IMPLEMENTATION_ID,
        DeviceDirection::Playback => ALSA_PLAYBACK_IMPLEMENTATION_ID,
    };
    if binding.implementation_id != expected {
        return Err(runtime_error(
            "CND-AUDDEV-011",
            "audio device exact implementation changed after resolution",
        ));
    }
    Ok(())
}

fn backend_name(node: &Node) -> Result<String, RuntimeError> {
    let resource = secret(node, "device_resource")
        .map_err(|error| runtime_error(error.code, error.message))?;
    [CAPTURE_RESOURCE_PREFIX, PLAYBACK_RESOURCE_PREFIX]
        .into_iter()
        .find_map(|prefix| resource.strip_prefix(prefix))
        .map(ToOwned::to_owned)
        .ok_or_else(|| runtime_error("CND-AUDDEV-004", "invalid exact ALSA device resource"))
}

fn verify_current_observation(node: &Node, direction: DeviceDirection) -> Result<(), RuntimeError> {
    let report = observe_alsa_devices(&AlsaToolchain::default(), 1, 12, 1_000_000);
    let observation = report.observation.ok_or_else(|| {
        runtime_error(
            report.reason_code,
            format!(
                "audio provider is not currently observed: {}",
                report.detail
            ),
        )
    })?;
    let resource = secret(node, "device_resource")
        .map_err(|error| runtime_error(error.code, error.message))?;
    if observation.observation_id != text(node, "provider_observation").unwrap_or_default()
        || observation.backend_identity != text(node, "backend_identity").unwrap_or_default()
        || observation.device(resource, direction).is_none()
    {
        return Err(runtime_error(
            "CND-AUDDEV-003",
            "audio device observation is stale or no longer names the selected endpoint",
        ));
    }
    Ok(())
}

enum CaptureMessage {
    Bytes(Vec<u8>),
    End,
    Failed(String),
}

struct AlsaCapture {
    binding: Option<ExactHostedServiceBinding>,
    child: Option<Child>,
    receiver: Option<Receiver<CaptureMessage>>,
    reader: Option<JoinHandle<()>>,
    next_frame: u64,
    waiting_before_first_sample: bool,
    lifecycle: AudioBoundaryLifecycle,
}

impl Default for AlsaCapture {
    fn default() -> Self {
        Self {
            binding: None,
            child: None,
            receiver: None,
            reader: None,
            next_frame: 0,
            waiting_before_first_sample: false,
            lifecycle: AudioBoundaryLifecycle::described(64),
        }
    }
}

impl Handler for AlsaCapture {
    fn prepare(
        &mut self,
        node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        validate_binding(node, &binding, DeviceDirection::Capture)?;
        self.binding = Some(binding);
        self.lifecycle.resolved();
        Ok(())
    }

    fn start(&mut self, node: &Node) -> Result<(), RuntimeError> {
        verify_current_observation(node, DeviceDirection::Capture)?;
        let device = backend_name(node)?;
        let frames = integer(node, "maximum_frames_per_step")
            .map_err(|error| runtime_error(error.code, error.message))?
            as usize;
        let bytes_per_chunk = frames * 2 * 2;
        let queue_frames = integer(node, "maximum_host_queue_frames")
            .map_err(|error| runtime_error(error.code, error.message))?
            as usize;
        let queue_chunks = (queue_frames / frames).max(1);
        let mut child = Command::new(DEFAULT_ARECORD_PATH)
            .args([
                "--device",
                &device,
                "--file-type",
                "raw",
                "--format",
                "S16_LE",
                "--channels",
                "2",
                "--rate",
                "48000",
                "--period-size",
                "32",
                "--buffer-size",
                "64",
                "--disable-resample",
                "--disable-channels",
                "--disable-format",
                "--disable-softvol",
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| map_open_error(error, DeviceDirection::Capture))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| runtime_error("CND-AUDDEV-011", "capture stdout is absent"))?;
        let (sender, receiver) = sync_channel(queue_chunks);
        let reader = thread::spawn(move || {
            loop {
                let mut bytes = vec![0_u8; bytes_per_chunk];
                match stdout.read_exact(&mut bytes) {
                    Ok(()) => {
                        if sender.send(CaptureMessage::Bytes(bytes)).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                        let _ = sender.send(CaptureMessage::End);
                        break;
                    }
                    Err(error) => {
                        let _ = sender.send(CaptureMessage::Failed(error.to_string()));
                        break;
                    }
                }
            }
        });
        self.child = Some(child);
        self.receiver = Some(receiver);
        self.reader = Some(reader);
        self.lifecycle.opened();
        self.lifecycle.started();
        Ok(())
    }

    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                "CND-AUDDEV-004",
                "audio capture received hidden graph input",
            ));
        }
        if !self.waiting_before_first_sample {
            self.waiting_before_first_sample = true;
            self.lifecycle.waiting();
            return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit.audio/alsa-capture-first-sample"),
                deadline_tick: context.tick.saturating_add(1),
            }));
        }
        let receiver = self
            .receiver
            .as_ref()
            .ok_or_else(|| runtime_error("CND-AUDDEV-011", "capture was not started"))?;
        match receiver.try_recv() {
            Ok(CaptureMessage::Bytes(bytes)) => {
                let samples = bytes
                    .chunks_exact(2)
                    .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
                    .collect::<Vec<_>>();
                let chunk = PcmChunk::new(
                    self.next_frame,
                    48_000,
                    ChannelLayout::StereoLr,
                    false,
                    samples,
                )
                .map_err(|reason| runtime_error("CND-AUDDEV-004", format!("{reason:?}")))?;
                self.next_frame = self.next_frame.saturating_add(chunk.frames() as u64);
                if !self
                    .lifecycle
                    .events()
                    .contains(&BoundaryEvent::FirstSample)
                {
                    self.lifecycle.first_sample();
                }
                Ok(HostedServiceStep::produced(vec![Value {
                    value_type: AUDIO_FRAME_TYPE,
                    bytes: encode_pcm_chunk(&chunk)
                        .map_err(|reason| runtime_error("CND-AUDDEV-004", format!("{reason:?}")))?,
                }]))
            }
            Ok(CaptureMessage::End) => {
                self.lifecycle.fail(BoundaryEvent::ProviderLost);
                Err(runtime_error(
                    "CND-AUDDEV-010",
                    "capture provider ended without an explicit lifecycle transition",
                ))
            }
            Ok(CaptureMessage::Failed(error)) => {
                self.lifecycle.fail(BoundaryEvent::ProviderLost);
                Err(runtime_error(
                    "CND-AUDDEV-011",
                    format!("capture provider failed: {error}"),
                ))
            }
            Err(TryRecvError::Empty) => {
                self.lifecycle.waiting();
                Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit.audio/alsa-capture-poll"),
                    deadline_tick: context.tick.saturating_add(1),
                }))
            }
            Err(TryRecvError::Disconnected) => {
                self.lifecycle.fail(BoundaryEvent::ProviderLost);
                Err(runtime_error(
                    "CND-AUDDEV-011",
                    "capture reader disconnected",
                ))
            }
        }
    }

    fn cancel(&mut self, _node: &Node, _stop: StopPolicy) -> Result<(), RuntimeError> {
        self.lifecycle.cancel();
        self.lifecycle.stop();
        if let Some(child) = &mut self.child {
            let _ = child.kill();
        }
        self.receiver.take();
        Ok(())
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        cleanup_process(
            &mut self.child,
            &mut self.reader,
            &mut self.lifecycle,
            context,
            "conduit.audio/alsa-capture-cleanup",
        )
    }
}

impl Drop for AlsaCapture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct AlsaPlayback {
    binding: Option<ExactHostedServiceBinding>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    next_frame: u64,
    lifecycle: AudioBoundaryLifecycle,
}

impl Default for AlsaPlayback {
    fn default() -> Self {
        Self {
            binding: None,
            child: None,
            stdin: None,
            next_frame: 0,
            lifecycle: AudioBoundaryLifecycle::described(64),
        }
    }
}

impl Handler for AlsaPlayback {
    fn prepare(
        &mut self,
        node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        validate_binding(node, &binding, DeviceDirection::Playback)?;
        self.binding = Some(binding);
        self.lifecycle.resolved();
        Ok(())
    }

    fn start(&mut self, node: &Node) -> Result<(), RuntimeError> {
        verify_current_observation(node, DeviceDirection::Playback)?;
        let device = backend_name(node)?;
        let mut child = Command::new(DEFAULT_APLAY_PATH)
            .args([
                "--device",
                &device,
                "--file-type",
                "raw",
                "--format",
                "S16_LE",
                "--channels",
                "2",
                "--rate",
                "48000",
                "--period-size",
                "32",
                "--buffer-size",
                "64",
                "--disable-resample",
                "--disable-channels",
                "--disable-format",
                "--disable-softvol",
            ])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| map_open_error(error, DeviceDirection::Playback))?;
        self.stdin = child.stdin.take();
        self.child = Some(child);
        self.lifecycle.opened();
        self.lifecycle.started();
        Ok(())
    }

    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            self.lifecycle.waiting();
            self.lifecycle.record(BoundaryEvent::Underrun);
            return Err(runtime_error(
                "CND-AUDDEV-007",
                "playback underrun is Waiting evidence, not clean audio",
            ));
        };
        let chunk = decode_pcm_chunk(input)
            .map_err(|reason| runtime_error("CND-AUDDEV-004", format!("{reason:?}")))?;
        if chunk.discontinuity || chunk.start_frame != self.next_frame {
            self.lifecycle.fail(BoundaryEvent::Discontinuity);
            return Err(runtime_error(
                "CND-AUDDEV-009",
                "playback discontinuity is terminal for the exact stream",
            ));
        }
        if chunk.frames() as u64
            > integer(node, "maximum_frames_per_step")
                .map_err(|error| runtime_error(error.code, error.message))?
        {
            self.lifecycle.fail(BoundaryEvent::Overrun);
            return Err(runtime_error(
                "CND-AUDDEV-008",
                "playback frame exceeds the admitted period",
            ));
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| runtime_error("CND-AUDDEV-011", "playback was not opened"))?;
        for sample in &chunk.samples {
            stdin
                .write_all(&sample.to_le_bytes())
                .map_err(|error| runtime_error("CND-AUDDEV-011", error.to_string()))?;
        }
        self.next_frame = self.next_frame.saturating_add(chunk.frames() as u64);
        if !self
            .lifecycle
            .events()
            .contains(&BoundaryEvent::FirstSample)
        {
            self.lifecycle.first_sample();
        }
        self.lifecycle.playback_committed();
        Ok(HostedServiceStep::produced(Vec::new()))
    }

    fn cancel(&mut self, _node: &Node, stop: StopPolicy) -> Result<(), RuntimeError> {
        self.lifecycle.cancel();
        if stop == StopPolicy::Drain {
            self.lifecycle.drain();
            self.stdin.take();
        } else {
            self.stdin.take();
            if let Some(child) = &mut self.child {
                let _ = child.kill();
            }
        }
        self.lifecycle.stop();
        Ok(())
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        let outcome = cleanup_process(
            &mut self.child,
            &mut None,
            &mut self.lifecycle,
            context,
            "conduit.audio/alsa-playback-cleanup",
        )?;
        if outcome == HostedServiceCleanup::Complete
            && self
                .lifecycle
                .events()
                .contains(&BoundaryEvent::DrainStarted)
        {
            self.lifecycle.drained();
        }
        Ok(outcome)
    }
}

impl Drop for AlsaPlayback {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn cleanup_process(
    child: &mut Option<Child>,
    reader: &mut Option<JoinHandle<()>>,
    lifecycle: &mut AudioBoundaryLifecycle,
    context: HostedServiceStepContext,
    subject: &'static str,
) -> Result<HostedServiceCleanup, RuntimeError> {
    if let Some(process) = child {
        match process.try_wait() {
            Ok(Some(_)) => {
                child.take();
            }
            Ok(None) => {
                let _ = process.kill();
                return Ok(HostedServiceCleanup::waiting(
                    HostedServiceInterest::Timer {
                        subject: Id(subject),
                        deadline_tick: context.tick.saturating_add(1),
                    },
                ));
            }
            Err(error) => {
                return Err(runtime_error(
                    "CND-AUDDEV-015",
                    format!("audio cleanup observation failed: {error}"),
                ));
            }
        }
    }
    if reader.as_ref().is_some_and(|thread| !thread.is_finished()) {
        return Ok(HostedServiceCleanup::waiting(
            HostedServiceInterest::Timer {
                subject: Id(subject),
                deadline_tick: context.tick.saturating_add(1),
            },
        ));
    }
    if let Some(thread) = reader.take() {
        thread
            .join()
            .map_err(|_| runtime_error("CND-AUDDEV-015", "audio reader cleanup panicked"))?;
    }
    lifecycle.close();
    Ok(HostedServiceCleanup::Complete)
}

fn map_open_error(error: io::Error, direction: DeviceDirection) -> RuntimeError {
    let code = match error.kind() {
        io::ErrorKind::PermissionDenied => "CND-AUDDEV-002",
        io::ErrorKind::NotFound => "CND-AUDDEV-001",
        io::ErrorKind::WouldBlock => "CND-AUDDEV-005",
        _ => "CND-AUDDEV-011",
    };
    runtime_error(
        code,
        format!("{direction:?} device open failed distinctly: {error}"),
    )
}

fn capture_factory() -> Box<dyn Handler> {
    Box::new(AlsaCapture::default())
}

fn playback_factory() -> Box<dyn Handler> {
    Box::new(AlsaPlayback::default())
}

pub fn register_observed_alsa_providers(
    registry: &mut Registry,
    report: &AlsaObservationReport,
) -> Result<(), RegistryError> {
    let Some(observation) = &report.observation else {
        return Err(RegistryError {
            code: report.reason_code,
            message: format!(
                "ALSA provider cannot be installed without a current device observation: {}",
                report.detail
            ),
        });
    };
    if report.state != ObservationState::Available
        || observation.arecord_path != Path::new(DEFAULT_ARECORD_PATH)
        || observation.aplay_path != Path::new(DEFAULT_APLAY_PATH)
    {
        return Err(RegistryError {
            code: "CND-AUDDEV-001",
            message: "ALSA provider requires the observed closed-inventory executable paths"
                .to_owned(),
        });
    }
    static CAPTURE_AUTHORITIES: [SemanticHash; 1] = [CAPTURE_AUTHORITY];
    static PLAYBACK_AUTHORITIES: [SemanticHash; 1] = [PLAYBACK_AUTHORITY];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: &AUDIO_CAPTURE_CONTRACT,
        implementation_id: ALSA_CAPTURE_IMPLEMENTATION_ID,
        artifact_id: "conduit.audio/capture-alsa-hosted-artifact",
        entrypoint: "audio-capture-alsa-hosted",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &CAPTURE_AUTHORITIES,
        factory: capture_factory,
        validate_config: validate_capture,
    })?;
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: &AUDIO_PLAYBACK_CONTRACT,
        implementation_id: ALSA_PLAYBACK_IMPLEMENTATION_ID,
        artifact_id: "conduit.audio/playback-alsa-hosted-artifact",
        entrypoint: "audio-playback-alsa-hosted",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &PLAYBACK_AUTHORITIES,
        factory: playback_factory,
        validate_config: validate_playback,
    })
}

/// Evaluates explicit ALSA device requests against one current host
/// observation and returns caller-owned authority facts for exact compilation.
/// This does not open a device or make an unavailable provider current.
pub fn observe_alsa_authorities(
    source: &str,
    report: &AlsaObservationReport,
    now_tick: u64,
    run_id: &str,
    epoch: u64,
) -> Result<Vec<ObservedHostServiceAuthority>, RuntimeError> {
    let observation = report.observation.as_ref().ok_or_else(|| {
        runtime_error(
            report.reason_code,
            format!("audio authority is unavailable: {}", report.detail),
        )
    })?;
    if report.state != ObservationState::Available {
        return Err(runtime_error(
            report.reason_code,
            "audio authority requires a current available provider observation",
        ));
    }
    let panel = conduit_panel::parse(source)
        .map_err(|error| runtime_error("CND-SRC-001", error.to_string()))?;
    let mut authorities = Vec::new();
    for node in &panel.nodes {
        let (contract_id, direction) = match node.kind.as_str() {
            "conduit.media/audio/capture" | "media/audio/capture" => {
                validate_capture(node).map_err(|error| runtime_error(error.code, error.message))?;
                ("conduit.media/audio/capture", DeviceDirection::Capture)
            }
            "conduit.media/audio/playback" | "media/audio/playback" => {
                validate_playback(node)
                    .map_err(|error| runtime_error(error.code, error.message))?;
                ("conduit.media/audio/playback", DeviceDirection::Playback)
            }
            _ => continue,
        };
        let resource_id = secret(node, "device_resource")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let grant_id = secret(node, "device_grant")
            .map_err(|error| runtime_error(error.code, error.message))?;
        if observation.device(resource_id, direction).is_none()
            || text(node, "provider_observation")
                .map_err(|error| runtime_error(error.code, error.message))?
                != observation.observation_id
            || text(node, "backend_identity")
                .map_err(|error| runtime_error(error.code, error.message))?
                != observation.backend_identity
            || integer(node, "observation_generation")
                .map_err(|error| runtime_error(error.code, error.message))?
                != observation.generation
            || integer(node, "observation_valid_until_tick")
                .map_err(|error| runtime_error(error.code, error.message))?
                != observation.valid_until_tick
            || !observation.is_fresh_at(now_tick)
        {
            return Err(runtime_error(
                "CND-AUDDEV-003",
                "audio authority request does not match the current host observation",
            ));
        }
        let constraints = observed_host_service_constraints(&[
            ("conduit.constraint/audio-device", resource_id.as_bytes()),
            (
                "conduit.constraint/audio-observation",
                observation.observation_id.as_bytes(),
            ),
            (
                "conduit.constraint/audio-backend",
                observation.backend_identity.as_bytes(),
            ),
        ]);
        let authority = observed_host_service_authority(HostServiceAuthorityObservationInput {
            contract_id: contract_id.to_owned(),
            instance: format!("root/{}", node.id),
            run_id: run_id.to_owned(),
            epoch,
            constraints,
            resource_id: Some(resource_id.to_owned()),
            grant_id: Some(grant_id.to_owned()),
            sharing: Some(
                text(node, "sharing_mode")
                    .map_err(|error| runtime_error(error.code, error.message))?
                    .to_owned(),
            ),
            maximum_holders: Some(
                integer(node, "maximum_concurrent_streams")
                    .map_err(|error| runtime_error(error.code, error.message))?
                    .try_into()
                    .map_err(|_| {
                        runtime_error("CND-AUDDEV-006", "audio holder limit exceeds u16")
                    })?,
            ),
            lease_ticks: Some(
                integer(node, "lease_ticks")
                    .map_err(|error| runtime_error(error.code, error.message))?,
            ),
            revocation_grace_ticks: Some(
                integer(node, "revocation_grace_ticks")
                    .map_err(|error| runtime_error(error.code, error.message))?,
            ),
            cleanup_ticks: Some(
                integer(node, "cleanup_ticks")
                    .map_err(|error| runtime_error(error.code, error.message))?,
            ),
        })
        .ok_or_else(|| runtime_error("CND-AUDDEV-017", "audio authority contract is absent"))?;
        authorities.push(authority);
    }
    Ok(authorities)
}

/// Renders one checked source using the exact currently observed ALSA null
/// endpoints. The returned source still requires explicit provider
/// installation and device grants; rendering it does not open either device.
pub fn render_observed_alsa_null_panel(
    observation: &AlsaObservation,
) -> Result<String, RuntimeError> {
    let capture = observation
        .device(
            "conduit.audio/device/alsa/capture/null",
            DeviceDirection::Capture,
        )
        .ok_or_else(|| runtime_error("CND-AUDDEV-001", "ALSA null capture is not observed"))?;
    let playback = observation
        .device(
            "conduit.audio/device/alsa/playback/null",
            DeviceDirection::Playback,
        )
        .ok_or_else(|| runtime_error("CND-AUDDEV-001", "ALSA null playback is not observed"))?;
    let capture_config = render_endpoint_config(
        capture,
        observation,
        CAPTURE_GRANT,
        "first-sample-delivered",
    );
    let playback_config = render_endpoint_config(
        playback,
        observation,
        PLAYBACK_GRANT,
        "backend-write-accepted",
    );
    Ok(format!(
        "panel 0\n\n# Exact observed hosted ALSA proof; no ambient default device is permitted.\ncapture: conduit.media/audio/capture {{\n{capture_config}}}\nprocess: conduit.media/audio/gain {{\n    lifecycle = \"standing\"\n    numeric_profile = \"pcm-s16-q15-round-nearest-away-saturate-no-nan-no-denormal-bit-exact\"\n    curve = \"linear-q15-absolute-frame\"\n    start_gain_q15 = 16384\n    end_gain_q15 = 16384\n    ramp_start_frame = 0\n    ramp_end_frame = 0\n    discontinuity = \"absolute-timeline\"\n    maximum_automation_points = 2\n    maximum_retained_samples = 0\n    maximum_frames = 32\n}}\nplayback: conduit.media/audio/playback {{\n{playback_config}}}\n\ncapture.frame > process.frame {{ capacity = 2 max_value_bytes = 152 max_queued_bytes = 304 low_watermark = 0 high_watermark = 2 pressure = block }}\nprocess.frame > playback.frame {{ capacity = 2 max_value_bytes = 152 max_queued_bytes = 304 low_watermark = 0 high_watermark = 2 pressure = block }}\n"
    ))
}

fn render_endpoint_config(
    device: &ObservedAudioDevice,
    observation: &AlsaObservation,
    grant: &str,
    commit_point: &str,
) -> String {
    let label = device
        .friendly_label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ");
    format!(
        "    device_resource = secret(\"{}\")\n    device_label = \"{label}\"\n    provider_observation = \"{}\"\n    observation_generation = {}\n    observation_valid_until_tick = {}\n    backend_identity = \"{}\"\n    sample_format = \"pcm-s16le-interleaved\"\n    sample_rate_hz = 48000\n    layout = \"stereo-lr\"\n    sample_clock = \"conduit.clock/alsa-device-48000\"\n    clock_correlation = \"observed-monotonic-uncertain\"\n    requested_period_frames = 32\n    admitted_period_frames = 32\n    requested_buffer_frames = 64\n    admitted_buffer_frames = 64\n    requested_latency_frames = 64\n    admitted_latency_frames = 64\n    latency_classification = \"observed\"\n    sharing_mode = \"shared-bounded\"\n    maximum_concurrent_streams = 1\n    workload_class = \"host-observed-best-effort\"\n    lifecycle = \"standing\"\n    underrun = \"wait-evidenced\"\n    overrun = \"fail-terminal-evidenced\"\n    drift = \"reject-evidenced\"\n    discontinuity = \"fail-terminal-evidenced\"\n    provider_loss = \"fail-terminal-evidenced\"\n    cancellation = \"before-open-after-open-running-drain-distinct\"\n    drain = \"flush-bounded\"\n    commit_point = \"{commit_point}\"\n    device_grant = secret(\"{grant}\")\n    lease_ticks = 1000\n    revocation_grace_ticks = 1\n    cleanup_ticks = 2\n    sensitivity = \"restricted-audio\"\n    maximum_frames_per_step = 32\n    maximum_host_queue_frames = 64\n    maximum_work = 64\n    maximum_evidence_events = 64\n",
        device.resource_id,
        observation.observation_id,
        observation.generation,
        observation.valid_until_tick,
        observation.backend_identity,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use conduit_compile::{InstalledProfile, compile_source};
    use conduit_core::{
        PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
        StepOutcomeKind,
    };
    use conduit_runtime::{
        ExactRunContext, ExactRunIo, ExactRunSessionRegistry, ExactRunState, SchedulerEventKind,
        SchedulerReservation, hosted_service_use_observations,
    };

    fn synthetic_available_report() -> AlsaObservationReport {
        AlsaObservationReport {
            state: ObservationState::Available,
            reason_code: "CND-AUDDEV-OK",
            detail: "synthetic closed observation".to_owned(),
            compiled_support: true,
            observation: Some(AlsaObservation {
                observation_id:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                generation: 1,
                observed_at_tick: 12,
                valid_until_tick: 112,
                backend_identity:
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_owned(),
                arecord_digest:
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_owned(),
                aplay_digest:
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                        .to_owned(),
                devices: vec![
                    ObservedAudioDevice {
                        direction: DeviceDirection::Capture,
                        resource_id: "conduit.audio/device/alsa/capture/null".to_owned(),
                        backend_name: "null".to_owned(),
                        friendly_label: "Synthetic null capture".to_owned(),
                    },
                    ObservedAudioDevice {
                        direction: DeviceDirection::Playback,
                        resource_id: "conduit.audio/device/alsa/playback/null".to_owned(),
                        backend_name: "null".to_owned(),
                        friendly_label: "Synthetic null playback".to_owned(),
                    },
                ],
                arecord_path: PathBuf::from(DEFAULT_ARECORD_PATH),
                aplay_path: PathBuf::from(DEFAULT_APLAY_PATH),
            }),
        }
    }

    #[test]
    fn lifecycle_distinguishes_every_backend_boundary() {
        let mut lifecycle = AudioBoundaryLifecycle::described(32);
        lifecycle.resolved();
        lifecycle.opened();
        lifecycle.started();
        lifecycle.waiting();
        lifecycle.first_sample();
        lifecycle.playback_committed();
        lifecycle.drain();
        lifecycle.drained();
        lifecycle.stop();
        lifecycle.close();
        assert_eq!(lifecycle.phase(), BoundaryPhase::Closed);
        for event in [
            BoundaryEvent::DescribedWithoutOpen,
            BoundaryEvent::ResolvedWithoutOpen,
            BoundaryEvent::Opened,
            BoundaryEvent::Started,
            BoundaryEvent::Waiting,
            BoundaryEvent::FirstSample,
            BoundaryEvent::PlaybackCommitted,
            BoundaryEvent::DrainStarted,
            BoundaryEvent::Drained,
            BoundaryEvent::Stopped,
            BoundaryEvent::Closed,
        ] {
            assert!(lifecycle.events().contains(&event), "missing {event:?}");
        }
    }

    #[test]
    fn cancellation_positions_and_dirty_audio_are_never_clean_completion() {
        let mut before = AudioBoundaryLifecycle::described(8);
        before.cancel();
        assert!(
            before
                .events()
                .contains(&BoundaryEvent::CancellationBeforeOpen)
        );

        let mut opened = AudioBoundaryLifecycle::described(8);
        opened.resolved();
        opened.opened();
        opened.cancel();
        assert!(
            opened
                .events()
                .contains(&BoundaryEvent::CancellationAfterOpen)
        );

        let mut draining = AudioBoundaryLifecycle::described(8);
        draining.resolved();
        draining.opened();
        draining.started();
        draining.drain();
        draining.cancel();
        assert!(
            draining
                .events()
                .contains(&BoundaryEvent::CancellationDuringDrain)
        );

        for fixture in [
            AudioDeviceFixture::Underrun,
            AudioDeviceFixture::Overrun,
            AudioDeviceFixture::ClockDrift,
            AudioDeviceFixture::HotUnplug,
            AudioDeviceFixture::ProviderRestart,
        ] {
            let outcome = fixture_outcome(fixture);
            assert!(!outcome.accepted);
            assert_ne!(outcome.terminal, "completed");
        }
    }

    #[test]
    fn complete_required_fixture_matrix_has_distinct_reasons() {
        let fixtures = AudioDeviceFixture::ALL;
        let mut reasons = fixtures
            .into_iter()
            .map(fixture_outcome)
            .map(|outcome| outcome.reason_code)
            .collect::<Vec<_>>();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), fixtures.len());
    }

    #[test]
    fn conformance_document_names_every_required_fixture_and_proof() {
        let document: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c4/audio-device-boundaries.json"
        ))
        .unwrap();
        assert_eq!(document["schema_version"], 0);
        let fixtures = document["fixtures"].as_array().unwrap();
        assert_eq!(fixtures.len(), AudioDeviceFixture::ALL.len());
        for fixture in AudioDeviceFixture::ALL {
            let outcome = fixture_outcome(fixture);
            assert!(fixtures.iter().any(|entry| {
                entry["id"] == fixture.id() && entry["code"] == outcome.reason_code
            }));
        }
        let proof_ids = document["proofs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|proof| proof["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            proof_ids,
            [
                "exact-plan-binding",
                "hosted-alsa-null-composition",
                "virtual-capture-standalone",
                "virtual-loopback-composition",
                "virtual-playback-standalone",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(document["presentation"]["ordered_text_equivalent"], true);
    }

    #[test]
    fn compiled_support_without_observation_is_honestly_unavailable() {
        let report = observe_alsa_devices(
            &AlsaToolchain {
                arecord_path: PathBuf::from("/definitely-absent/conduit-arecord"),
                aplay_path: PathBuf::from("/definitely-absent/conduit-aplay"),
            },
            1,
            12,
            100,
        );
        assert!(report.compiled_support);
        assert_eq!(report.state, ObservationState::Unavailable);
        assert!(report.observation.is_none());
        assert_eq!(report.reason_code, "CND-AUDDEV-001");
    }

    #[test]
    fn permission_staleness_and_source_authorship_fail_closed() {
        let denied = observation_error(
            io::Error::new(io::ErrorKind::PermissionDenied, "fixture denied"),
            "fixture observation",
        );
        assert_eq!(denied.state, ObservationState::PermissionDenied);
        assert_eq!(denied.reason_code, "CND-AUDDEV-002");
        assert!(denied.observation.is_none());

        let report = synthetic_available_report();
        let source = render_observed_alsa_null_panel(report.observation.as_ref().unwrap()).unwrap();
        let stale = observe_alsa_authorities(&source, &report, 113, "run/audio", 1).unwrap_err();
        assert_eq!(stale.code, "CND-AUDDEV-003");

        let mut registry = Registry::hosted_primitives();
        conduit_media::register_deterministic_audio_processing_providers(&mut registry).unwrap();
        register_observed_alsa_providers(&mut registry, &report).unwrap();
        let source_only = InstalledProfile::observe_registry(&source, &registry).unwrap();
        assert_eq!(
            compile_source(&source, &source_only.input)
                .unwrap_err()
                .code(),
            "CND-CMP-006",
            "source and provider installation cannot satisfy audio device authority"
        );
    }

    #[test]
    fn live_observation_does_not_open_a_device_and_excludes_ambient_defaults() {
        let report = observe_alsa_devices(&AlsaToolchain::default(), 1, 12, 100);
        if report.state != ObservationState::Available {
            assert!(report.observation.is_none());
            return;
        }
        let observation = report.observation.unwrap();
        assert!(observation.is_fresh_at(12));
        assert!(!observation.is_fresh_at(113));
        assert!(observation.devices.iter().all(|device| {
            device.backend_name == "null" || device.backend_name.starts_with("hw:")
        }));
        assert!(observation.devices.iter().all(|device| {
            !device.backend_name.contains("default") && device.resource_id != device.friendly_label
        }));
    }

    #[test]
    fn observed_alsa_panel_seals_devices_grants_backend_and_limits_into_one_exact_plan() {
        let report = observe_alsa_devices(&AlsaToolchain::default(), 1, 12, 1000);
        if report.state != ObservationState::Available {
            assert!(report.observation.is_none());
            return;
        }
        let observation = report.observation.as_ref().unwrap();
        if observation
            .device(
                "conduit.audio/device/alsa/capture/null",
                DeviceDirection::Capture,
            )
            .is_none()
            || observation
                .device(
                    "conduit.audio/device/alsa/playback/null",
                    DeviceDirection::Playback,
                )
                .is_none()
        {
            return;
        }
        let source = render_observed_alsa_null_panel(observation).unwrap();
        let mut registry = Registry::hosted_primitives();
        conduit_media::register_deterministic_audio_processing_providers(&mut registry).unwrap();
        register_observed_alsa_providers(&mut registry, &report).unwrap();
        let absent = InstalledProfile::observe_registry(&source, &registry).unwrap();
        assert_eq!(
            compile_source(&source, &absent.input).unwrap_err().code(),
            "CND-CMP-006"
        );
        let authorities =
            observe_alsa_authorities(&source, &report, 12, "conduit/conduct-run", 1).unwrap();
        let installed = InstalledProfile::observe_registry_with_host_authorities(
            &source,
            &registry,
            &authorities,
        )
        .unwrap();
        let plan = compile_source(&source, &installed.input).unwrap();
        assert!(plan.nodes.iter().any(|node| {
            node.contract.id == "conduit.media/audio/capture"
                && node.implementation.id == ALSA_CAPTURE_IMPLEMENTATION_ID
                && node.artifact == "conduit.audio/capture-alsa-hosted-artifact"
        }));
        assert!(plan.nodes.iter().any(|node| {
            node.contract.id == "conduit.media/audio/playback"
                && node.implementation.id == ALSA_PLAYBACK_IMPLEMENTATION_ID
                && node.artifact == "conduit.audio/playback-alsa-hosted-artifact"
        }));
        assert_eq!(plan.authorities.len(), 2);
        assert!(plan.authorities.iter().any(|authority| {
            authority.grant.id == CAPTURE_GRANT
                && authority.binding.resource_id == "conduit.audio/device/alsa/capture/null"
        }));
        assert!(plan.authorities.iter().any(|authority| {
            authority.grant.id == PLAYBACK_GRANT
                && authority.binding.resource_id == "conduit.audio/device/alsa/playback/null"
        }));
        assert!(plan.resources.iter().all(|resource| {
            resource.lease.as_ref().is_some_and(|lease| {
                lease.sharing == "shared-bounded"
                    && lease.maximum_holders == 1
                    && lease.revocation_grace_ticks == 1
                    && lease.cleanup_ticks == 2
            })
        }));
        assert!(
            plan.nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.contract.id.as_str(),
                        "conduit.media/audio/capture" | "conduit.media/audio/playback"
                    )
                })
                .all(|node| {
                    node.execution_profile.boundedness == "observed"
                        && !node.execution_profile.step_bound_enforced
                        && node.execution_profile.limits.max_foreign_queue_bytes == 4096
                })
        );
    }

    #[test]
    fn observed_alsa_null_runs_capture_processing_playback_through_the_production_executor() {
        let report = observe_alsa_devices(&AlsaToolchain::default(), 1, 12, 1_000_000);
        let Some(observation) = report.observation.as_ref() else {
            assert_ne!(report.state, ObservationState::Available);
            return;
        };
        if observation
            .device(
                "conduit.audio/device/alsa/capture/null",
                DeviceDirection::Capture,
            )
            .is_none()
            || observation
                .device(
                    "conduit.audio/device/alsa/playback/null",
                    DeviceDirection::Playback,
                )
                .is_none()
        {
            return;
        }
        let source = render_observed_alsa_null_panel(observation).unwrap();
        let mut registry = Registry::hosted_primitives();
        conduit_media::register_deterministic_audio_processing_providers(&mut registry).unwrap();
        register_observed_alsa_providers(&mut registry, &report).unwrap();
        let authorities =
            observe_alsa_authorities(&source, &report, 12, "run/audio/alsa-null", 147).unwrap();
        let installed = InstalledProfile::observe_registry_with_host_authorities(
            &source,
            &registry,
            &authorities,
        )
        .unwrap();
        let document = compile_source(&source, &installed.input).unwrap();
        let arena = Bump::new();
        let plan = document.as_plan(&arena).unwrap();
        let panel = conduit_panel::parse(&source).unwrap();
        let resolved = registry.resolve(&panel).unwrap();
        let bindings = installed.bindings(&plan).unwrap();
        let grants = installed.grant_observations(&plan).unwrap();
        let use_observations = hosted_service_use_observations(&grants);
        let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
        let mut session = resolved
            .start_exact_session(
                &plan,
                &bindings,
                ExactRunContext {
                    semantic_source_hash: plan.source_semantic_hash,
                    plan_epoch: 147,
                    run_id: Id("run/audio/alsa-null"),
                    grant_observations: &grants,
                    validation: PlanValidationContext {
                        supported_schema_version: plan.schema_version,
                        now: plan.created_at,
                    },
                    scheduler_policy: SchedulerPolicy {
                        schema_version: SCHEDULER_CONTRACT_VERSION,
                        ready_queue: ReadyQueueDiscipline::RoundRobin,
                        max_decisions: 256,
                        max_tick: 256,
                        max_consecutive_yields: 16,
                        max_events: 256,
                    },
                    reservation: SchedulerReservation {
                        available_runtime_memory_bytes: plan.budget.memory_bytes,
                        executor_overhead_limit_bytes: plan.budget.memory_bytes,
                    },
                },
                &sessions,
                ExactRunIo::for_plan(&plan).unwrap(),
            )
            .unwrap();

        let pump = session.pump(16, &use_observations).unwrap();
        let saw_waiting = session.scheduler_events().any(|event| {
            matches!(
                event.kind,
                SchedulerEventKind::NodeOutcome {
                    outcome: StepOutcomeKind::Pending
                }
            )
        });
        assert!(
            saw_waiting,
            "hosted audio exposes a pending wait rather than clean completion"
        );
        let progressed_nodes = session
            .scheduler_events()
            .filter(|event| {
                matches!(
                    event.kind,
                    SchedulerEventKind::NodeOutcome {
                        outcome: StepOutcomeKind::Progress
                    }
                )
            })
            .count();
        assert!(
            progressed_nodes >= 3,
            "capture, bounded gain, and playback each commit scheduler progress"
        );
        assert!(!matches!(pump.state, ExactRunState::Terminal(_)));
        let cancelled = session.cancel(StopPolicy::Abort).unwrap();
        assert!(matches!(
            cancelled.state,
            ExactRunState::Aborting | ExactRunState::Terminal(_)
        ));
    }
}
