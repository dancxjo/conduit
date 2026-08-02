//! Host adapters for published audio contracts.
//!
//! The provider names in this module are implementation identities, not node
//! identities. Every adapter registers through the generic runtime
//! `InstalledImplementationRegistration`; resolver, manifest, artifact,
//! authority, exact-plan, and host-conformance machinery remain domain-neutral.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use conduit_core::{ArtifactDigest, ExecutorKind, Id, PinnedDescriptor, SemanticHash};
use conduit_media::{
    AUDIO_GAIN_CONTRACT, AudioProcessingReason, ChannelLayout, MAXIMUM_PCM_FRAMES, PcmChunk,
    REFERENCE_NUMERIC_PROFILE, decode_pcm_chunk, encode_pcm_chunk,
    register_audio_processing_contracts,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    ExactHostedServiceBinding, Handler, HostedServiceStep, HostedServiceStepContext,
    InstalledArtifactRegistration, InstalledImplementationRegistration, Registry, RegistryError,
    ResolutionError, RunIo, RuntimeError, Value,
};
use sha2::{Digest, Sha256};

#[cfg(not(target_arch = "wasm32"))]
use conduit_process::{
    SupervisedProcessCancellation, SupervisedProcessLimits, SupervisedProcessRequest,
    SupervisedProcessTerminal, run_supervised_process,
};
#[cfg(not(target_arch = "wasm32"))]
use std::ffi::OsString;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;

pub const FFMPEG_EXECUTABLE: &str = "/usr/bin/ffmpeg";
pub const SOX_EXECUTABLE: &str = "/usr/bin/sox";
pub const HALF_GAIN_Q15: u32 = 16_384;
pub const MAXIMUM_VERSION_BYTES: usize = 16 * 1024;
pub const MAXIMUM_PROCESS_OUTPUT_BYTES: usize = 4 * 1024;
pub const PROCESS_DEADLINE_MILLIS: u64 = 2_000;
pub const PROCESS_CLEANUP_MILLIS: u64 = 250;

pub const PROCESS_EXECUTE_AUTHORITY: SemanticHash = SemanticHash::from_bytes([0x71; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaImplementation {
    FfmpegProcess,
    SoxProcess,
    BrowserWasmLinked,
}

impl MediaImplementation {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::FfmpegProcess => "conduit.media/audio-gain-ffmpeg-process",
            Self::SoxProcess => "conduit.media/audio-gain-sox-process",
            Self::BrowserWasmLinked => "conduit.media/audio-gain-browser-wasm-linked",
        }
    }

    #[must_use]
    pub const fn artifact_id(self) -> &'static str {
        match self {
            Self::FfmpegProcess => "conduit.media/ffmpeg-executable",
            Self::SoxProcess => "conduit.media/sox-executable",
            Self::BrowserWasmLinked => "conduit.media/audio-gain-browser-wasm-linked-artifact",
        }
    }

    #[must_use]
    pub const fn boundary(self) -> conduit_core::ProviderBoundary {
        match self {
            Self::FfmpegProcess | Self::SoxProcess => {
                conduit_core::ProviderBoundary::SupervisedProcess
            }
            Self::BrowserWasmLinked => conduit_core::ProviderBoundary::WasmBrowser,
        }
    }

    const fn executor(self) -> ExecutorKind {
        match self {
            Self::FfmpegProcess | Self::SoxProcess => ExecutorKind::Process,
            // This adapter is linked into the browser host's Rust/WASM
            // executable and invoked through its ordinary bounded executor.
            Self::BrowserWasmLinked => ExecutorKind::NativeInProcess,
        }
    }

    const fn is_process(self) -> bool {
        matches!(self, Self::FfmpegProcess | Self::SoxProcess)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaImplementationLimits {
    pub maximum_processes: usize,
    pub maximum_child_processes: usize,
    pub maximum_threads: usize,
    pub maximum_descriptors: usize,
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub maximum_pending_callbacks: usize,
    pub maximum_foreign_queue: usize,
    pub maximum_temp_bytes: usize,
    pub maximum_duration_millis: u64,
    pub maximum_cleanup_millis: u64,
    pub maximum_evidence_events: usize,
}

impl MediaImplementationLimits {
    #[must_use]
    pub const fn process() -> Self {
        Self {
            maximum_processes: 1,
            maximum_child_processes: 0,
            maximum_threads: 8,
            maximum_descriptors: 16,
            maximum_input_bytes: 128,
            maximum_output_bytes: 128,
            maximum_stderr_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
            maximum_pending_callbacks: 0,
            maximum_foreign_queue: 0,
            maximum_temp_bytes: 0,
            maximum_duration_millis: PROCESS_DEADLINE_MILLIS,
            maximum_cleanup_millis: PROCESS_CLEANUP_MILLIS,
            maximum_evidence_events: 16,
        }
    }

    #[must_use]
    pub const fn browser() -> Self {
        Self {
            maximum_processes: 0,
            maximum_child_processes: 0,
            maximum_threads: 1,
            maximum_descriptors: 0,
            maximum_input_bytes: 128,
            maximum_output_bytes: 128,
            maximum_stderr_bytes: 0,
            maximum_pending_callbacks: 1,
            maximum_foreign_queue: 1,
            maximum_temp_bytes: 0,
            maximum_duration_millis: 50,
            maximum_cleanup_millis: 50,
            maximum_evidence_events: 16,
        }
    }
}

/// Exact executable/artifact observation produced by a host observer.
///
/// This is adapter input, not semantic source and not authority. The generic
/// host-conformance `ProviderObservation` remains the canonical availability
/// record used for binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedMediaArtifact {
    pub implementation: MediaImplementation,
    pub digest: ArtifactDigest,
    pub byte_size: u64,
    pub executable: Option<PathBuf>,
    pub version: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub limits: MediaImplementationLimits,
}

impl ObservedMediaArtifact {
    pub fn browser_wasm_linked(
        artifact: &[u8],
        observed_at_tick: u64,
        valid_until_tick: u64,
    ) -> Result<Self, MediaImplementationError> {
        if artifact.is_empty() || valid_until_tick <= observed_at_tick {
            return Err(MediaImplementationError::InvalidObservation);
        }
        Ok(Self {
            implementation: MediaImplementation::BrowserWasmLinked,
            digest: ArtifactDigest::from_bytes(Sha256::digest(artifact).into()),
            byte_size: artifact.len() as u64,
            executable: None,
            version: "bounded-q15-browser-wasm-linked".to_owned(),
            observed_at_tick,
            valid_until_tick,
            limits: MediaImplementationLimits::browser(),
        })
    }

    #[must_use]
    pub fn fresh_at(&self, tick: u64) -> bool {
        self.observed_at_tick <= tick && tick < self.valid_until_tick
    }

    #[must_use]
    pub fn redacted_summary(&self) -> String {
        let artifact = self
            .executable
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("browser-wasm-linked");
        format!(
            "implementation={};artifact={artifact};digest={};valid={}..={}",
            self.implementation.id(),
            self.digest,
            self.observed_at_tick,
            self.valid_until_tick
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaImplementationError {
    UnsupportedTarget,
    InvalidObservation,
    ExecutableAbsent,
    ExecutableNotAbsolute,
    ExecutableChanged,
    VersionProbeFailed,
    VersionOutputOverflow,
    UnsupportedVersion,
    ObservationStale,
    UnsupportedProfile,
    MalformedInput,
    InputOverflow,
    OutputOverflow,
    StderrOverflow,
    SpawnFailed,
    ProcessFailed,
    DeadlineExceeded,
    Cancelled,
    CleanupFailed,
    InexactBinding,
}

impl MediaImplementationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedTarget => "CND-MPR-001",
            Self::InvalidObservation => "CND-MPR-002",
            Self::ExecutableAbsent => "CND-MPR-003",
            Self::ExecutableNotAbsolute => "CND-MPR-004",
            Self::ExecutableChanged => "CND-MPR-005",
            Self::VersionProbeFailed => "CND-MPR-006",
            Self::VersionOutputOverflow => "CND-MPR-007",
            Self::UnsupportedVersion => "CND-MPR-008",
            Self::ObservationStale => "CND-MPR-009",
            Self::UnsupportedProfile => "CND-MPR-010",
            Self::MalformedInput => "CND-MPR-011",
            Self::InputOverflow => "CND-MPR-012",
            Self::OutputOverflow => "CND-MPR-013",
            Self::StderrOverflow => "CND-MPR-014",
            Self::SpawnFailed => "CND-MPR-015",
            Self::ProcessFailed => "CND-MPR-016",
            Self::DeadlineExceeded => "CND-MPR-017",
            Self::Cancelled => "CND-MPR-018",
            Self::CleanupFailed => "CND-MPR-019",
            Self::InexactBinding => "CND-MPR-020",
        }
    }
}

impl std::fmt::Display for MediaImplementationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for MediaImplementationError {}

#[cfg(not(target_arch = "wasm32"))]
pub fn observe_media_executable(
    implementation: MediaImplementation,
    executable: &Path,
    observed_at_tick: u64,
    valid_until_tick: u64,
) -> Result<ObservedMediaArtifact, MediaImplementationError> {
    if !implementation.is_process() || valid_until_tick <= observed_at_tick {
        return Err(MediaImplementationError::InvalidObservation);
    }
    if !executable.is_absolute() {
        return Err(MediaImplementationError::ExecutableNotAbsolute);
    }
    let executable =
        fs::canonicalize(executable).map_err(|_| MediaImplementationError::ExecutableAbsent)?;
    let bytes = fs::read(&executable).map_err(|_| MediaImplementationError::ExecutableAbsent)?;
    if bytes.is_empty() {
        return Err(MediaImplementationError::ExecutableAbsent);
    }
    let argument = match implementation {
        MediaImplementation::FfmpegProcess => "-version",
        MediaImplementation::SoxProcess => "--version",
        MediaImplementation::BrowserWasmLinked => unreachable!(),
    };
    let version = bounded_version_probe(&executable, argument)?;
    let version =
        String::from_utf8(version).map_err(|_| MediaImplementationError::VersionProbeFailed)?;
    let first_line = version.lines().next().unwrap_or_default();
    let expected = match implementation {
        MediaImplementation::FfmpegProcess => "ffmpeg version",
        MediaImplementation::SoxProcess => "SoX",
        MediaImplementation::BrowserWasmLinked => unreachable!(),
    };
    if !first_line.contains(expected) {
        return Err(MediaImplementationError::UnsupportedVersion);
    }
    Ok(ObservedMediaArtifact {
        implementation,
        digest: ArtifactDigest::from_bytes(Sha256::digest(&bytes).into()),
        byte_size: bytes.len() as u64,
        executable: Some(executable),
        version: first_line.to_owned(),
        observed_at_tick,
        valid_until_tick,
        limits: MediaImplementationLimits::process(),
    })
}

#[cfg(target_arch = "wasm32")]
pub fn observe_media_executable(
    _implementation: MediaImplementation,
    _executable: &Path,
    _observed_at_tick: u64,
    _valid_until_tick: u64,
) -> Result<ObservedMediaArtifact, MediaImplementationError> {
    Err(MediaImplementationError::UnsupportedTarget)
}

#[cfg(not(target_arch = "wasm32"))]
fn bounded_version_probe(
    executable: &Path,
    argument: &str,
) -> Result<Vec<u8>, MediaImplementationError> {
    let limits = SupervisedProcessLimits {
        maximum_arguments: 1,
        maximum_stdin_bytes: 1,
        maximum_stdout_bytes: MAXIMUM_VERSION_BYTES,
        maximum_stderr_bytes: MAXIMUM_VERSION_BYTES,
        maximum_processes: 1,
        maximum_child_processes: 0,
        maximum_threads: 8,
        maximum_descriptors: 16,
        deadline_millis: PROCESS_DEADLINE_MILLIS,
        cleanup_millis: PROCESS_CLEANUP_MILLIS,
    };
    let argv = [OsString::from(argument)];
    let result = run_supervised_process(&SupervisedProcessRequest {
        executable,
        argv: &argv,
        environment: &[],
        working_directory: Path::new("/"),
        stdin: &[],
        limits,
        cancellation: SupervisedProcessCancellation::None,
    })
    .map_err(|error| match error {
        conduit_process::SupervisedProcessError::OutputOverflow
        | conduit_process::SupervisedProcessError::StderrOverflow => {
            MediaImplementationError::VersionOutputOverflow
        }
        _ => MediaImplementationError::VersionProbeFailed,
    })?;
    if result.terminal != SupervisedProcessTerminal::Exited(0) {
        return Err(MediaImplementationError::VersionProbeFailed);
    }
    if result.stdout.is_empty() {
        Ok(result.stderr)
    } else {
        Ok(result.stdout)
    }
}

static FFMPEG: OnceLock<ObservedMediaArtifact> = OnceLock::new();
static SOX: OnceLock<ObservedMediaArtifact> = OnceLock::new();

/// Install one media implementation through the generic implementation path.
pub fn install_audio_gain_implementation(
    registry: &mut Registry,
    observed: ObservedMediaArtifact,
) -> Result<(), RegistryError> {
    register_audio_processing_contracts(registry);
    if observed.implementation.is_process() {
        let slot = match observed.implementation {
            MediaImplementation::FfmpegProcess => &FFMPEG,
            MediaImplementation::SoxProcess => &SOX,
            MediaImplementation::BrowserWasmLinked => unreachable!(),
        };
        if slot.set(observed.clone()).is_err() && slot.get() != Some(&observed) {
            return Err(RegistryError {
                code: "CND-MPR-002",
                message: "implementation was already installed from another observation".to_owned(),
            });
        }
    }
    let implementation = observed.implementation;
    let profile = match implementation {
        MediaImplementation::FfmpegProcess | MediaImplementation::SoxProcess => {
            "conduit/media-supervised-process-profile"
        }
        MediaImplementation::BrowserWasmLinked => "conduit/media-browser-wasm-linked-profile",
    };
    let (target, abi, media_type, builder, factory) = match implementation {
        MediaImplementation::FfmpegProcess => (
            std::env::consts::ARCH,
            "conduit/supervised-process",
            "application/vnd.conduit.observed-executable",
            "external/observed-artifact",
            ffmpeg_handler as conduit_runtime::HandlerFactory,
        ),
        MediaImplementation::SoxProcess => (
            std::env::consts::ARCH,
            "conduit/supervised-process",
            "application/vnd.conduit.observed-executable",
            "external/observed-artifact",
            sox_handler as conduit_runtime::HandlerFactory,
        ),
        MediaImplementation::BrowserWasmLinked => (
            "wasm32",
            "conduit/rust-wasm-in-process",
            "application/vnd.conduit.compiled-in-provider",
            "conduit/rustc-workspace-build",
            browser_handler as conduit_runtime::HandlerFactory,
        ),
    };
    registry.register_installed_implementation(InstalledImplementationRegistration {
        contract: &AUDIO_GAIN_CONTRACT,
        implementation_id: implementation.id().to_owned(),
        implementation_version: observed.version.clone(),
        executor: implementation.executor(),
        entrypoint_name: format!("{}-step", implementation.id()),
        entrypoint_adapter: match implementation {
            MediaImplementation::BrowserWasmLinked => "conduit/browser-wasm-linked-step".to_owned(),
            MediaImplementation::FfmpegProcess | MediaImplementation::SoxProcess => {
                "conduit/supervised-process-step".to_owned()
            }
        },
        entrypoint_abi: abi.to_owned(),
        entrypoint_protocol_version: 0,
        execution_profile: PinnedDescriptor {
            id: Id(profile),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes(Sha256::digest(profile).into()),
        },
        artifacts: vec![InstalledArtifactRegistration {
            id: implementation.artifact_id().to_owned(),
            digest: observed.digest,
            media_type: media_type.to_owned(),
            byte_size: observed.byte_size,
            target: Some(target.to_owned()),
            abi: Some(abi.to_owned()),
            builder: builder.to_owned(),
            source_digest: observed.digest,
            build_recipe_digest: ArtifactDigest::from_bytes(
                Sha256::digest(observed.version.as_bytes()).into(),
            ),
            reproducible: implementation == MediaImplementation::BrowserWasmLinked,
            license_expressions: Vec::new(),
            role: "executable".to_owned(),
            required: true,
        }],
        required_authorities: implementation
            .is_process()
            .then_some(PROCESS_EXECUTE_AUTHORITY)
            .into_iter()
            .collect(),
        required_effects: Vec::new(),
        minimum_plan_version: 0,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        coexistence_memory_bytes: 0,
        managed_lifecycle: None,
        factory,
        validate_config: validate_constant_half_gain,
    })
}

fn integer(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    let Some(SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-MPR-010",
            format!("implementation configuration `{key}` must be an integer"),
        ));
    };
    u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-MPR-010",
            format!("implementation configuration `{key}` must be nonnegative"),
        )
    })
}

fn validate_constant_half_gain(node: &Node) -> Result<(), ResolutionError> {
    let exact = node.config.len() == 11
        && matches!(node.config("lifecycle"), Some("finite") | Some("standing"))
        && node.config("numeric_profile") == Some(REFERENCE_NUMERIC_PROFILE)
        && node.config("curve") == Some("linear-q15-absolute-frame")
        && node.config("discontinuity") == Some("absolute-timeline")
        && integer(node, "start_gain_q15")? == u64::from(HALF_GAIN_Q15)
        && integer(node, "end_gain_q15")? == u64::from(HALF_GAIN_Q15)
        && integer(node, "ramp_start_frame")? == 0
        && integer(node, "ramp_end_frame")? == 0
        && integer(node, "maximum_automation_points")? == 2
        && integer(node, "maximum_retained_samples")? == 0
        && (1..=MAXIMUM_PCM_FRAMES as u64).contains(&integer(node, "maximum_frames")?);
    exact.then_some(()).ok_or_else(|| {
        ResolutionError::new(
            "CND-MPR-010",
            "implementation supports only exact stereo 48 kHz constant half-gain Q15",
        )
    })
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct ProcessGainHandler {
    implementation: MediaImplementation,
    binding: Option<ExactHostedServiceBinding>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Handler for ProcessGainHandler {
    fn bind_exact(&mut self, binding: ExactHostedServiceBinding) -> Result<(), RuntimeError> {
        if binding.implementation_id != self.implementation.id()
            || binding.artifact_id != self.implementation.artifact_id()
            || binding.authorities.is_empty()
        {
            return Err(runtime_error(MediaImplementationError::InexactBinding));
        }
        self.binding = Some(binding);
        Ok(())
    }

    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if self.binding.is_none() {
            return Err(runtime_error(MediaImplementationError::InexactBinding));
        }
        let [input] = inputs else {
            return Err(runtime_error(MediaImplementationError::MalformedInput));
        };
        let input = decode_pcm_chunk(input).map_err(audio_error)?;
        let observed = match self.implementation {
            MediaImplementation::FfmpegProcess => FFMPEG.get(),
            MediaImplementation::SoxProcess => SOX.get(),
            MediaImplementation::BrowserWasmLinked => None,
        }
        .ok_or_else(|| runtime_error(MediaImplementationError::InvalidObservation))?;
        // Exact-run admission has already validated the plan's host
        // observation against its authority time. Scheduler ticks are a
        // different clock and must never be reused as observation time.
        let _scheduler_tick = context.tick;
        let output =
            execute_process_gain_bound(observed, &input, SupervisedProcessCancellation::None)?
                .ok_or_else(|| runtime_error(MediaImplementationError::Cancelled))?;
        let value = pcm_value(&output)?;
        Ok(if node.config("lifecycle") == Some("standing") {
            HostedServiceStep::produced(vec![value])
        } else {
            HostedServiceStep::completed(vec![value])
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl Handler for ProcessGainHandler {
    fn step(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        Err(runtime_error(MediaImplementationError::UnsupportedTarget))
    }
}

struct BrowserGainHandler;

impl Handler for BrowserGainHandler {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(MediaImplementationError::MalformedInput));
        };
        let mut chunk = decode_pcm_chunk(input).map_err(audio_error)?;
        validate_chunk(&chunk)?;
        // Independent bounded implementation of the published Q15
        // round-nearest-away profile. No reference-provider routine or
        // provider-specific value type crosses this adapter.
        for sample in &mut chunk.samples {
            let widened = i32::from(*sample);
            *sample = if widened >= 0 {
                ((widened + 1) / 2) as i16
            } else {
                ((widened - 1) / 2) as i16
            };
        }
        let value = pcm_value(&chunk)?;
        Ok(if node.config("lifecycle") == Some("standing") {
            HostedServiceStep::produced(vec![value])
        } else {
            HostedServiceStep::completed(vec![value])
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn execute_process_gain(
    observed: &ObservedMediaArtifact,
    tick: u64,
    input: &PcmChunk,
    cancellation: SupervisedProcessCancellation,
) -> Result<Option<PcmChunk>, MediaImplementationError> {
    if !observed.implementation.is_process() {
        return Err(MediaImplementationError::UnsupportedTarget);
    }
    if !observed.fresh_at(tick) {
        return Err(MediaImplementationError::ObservationStale);
    }
    execute_process_gain_bound(observed, input, cancellation)
}

#[cfg(not(target_arch = "wasm32"))]
fn execute_process_gain_bound(
    observed: &ObservedMediaArtifact,
    input: &PcmChunk,
    cancellation: SupervisedProcessCancellation,
) -> Result<Option<PcmChunk>, MediaImplementationError> {
    validate_chunk(input)?;
    let executable = observed
        .executable
        .as_deref()
        .ok_or(MediaImplementationError::ExecutableAbsent)?;
    let bytes = fs::read(executable).map_err(|_| MediaImplementationError::ExecutableAbsent)?;
    if ArtifactDigest::from_bytes(Sha256::digest(bytes).into()) != observed.digest {
        return Err(MediaImplementationError::ExecutableChanged);
    }
    let mut raw = Vec::with_capacity(input.samples.len() * 2);
    for sample in &input.samples {
        raw.extend_from_slice(&sample.to_le_bytes());
    }
    if raw.len() > observed.limits.maximum_input_bytes {
        return Err(MediaImplementationError::InputOverflow);
    }
    let argv = process_argv(observed.implementation);
    let result = run_supervised_process(&SupervisedProcessRequest {
        executable,
        argv: &argv,
        environment: &[],
        working_directory: Path::new("/"),
        stdin: &raw,
        limits: SupervisedProcessLimits {
            maximum_arguments: argv.len(),
            maximum_stdin_bytes: observed.limits.maximum_input_bytes,
            maximum_stdout_bytes: observed.limits.maximum_output_bytes,
            maximum_stderr_bytes: observed.limits.maximum_stderr_bytes,
            maximum_processes: observed.limits.maximum_processes,
            maximum_child_processes: observed.limits.maximum_child_processes,
            maximum_threads: observed.limits.maximum_threads,
            maximum_descriptors: observed.limits.maximum_descriptors,
            deadline_millis: observed.limits.maximum_duration_millis,
            cleanup_millis: observed.limits.maximum_cleanup_millis,
        },
        cancellation,
    })
    .map_err(|error| match error {
        conduit_process::SupervisedProcessError::InputOverflow => {
            MediaImplementationError::InputOverflow
        }
        conduit_process::SupervisedProcessError::OutputOverflow => {
            MediaImplementationError::OutputOverflow
        }
        conduit_process::SupervisedProcessError::StderrOverflow => {
            MediaImplementationError::StderrOverflow
        }
        conduit_process::SupervisedProcessError::SpawnFailed => {
            MediaImplementationError::SpawnFailed
        }
        conduit_process::SupervisedProcessError::CleanupFailed => {
            MediaImplementationError::CleanupFailed
        }
        _ => MediaImplementationError::ProcessFailed,
    })?;
    match result.terminal {
        SupervisedProcessTerminal::Cancelled { .. } => return Ok(None),
        SupervisedProcessTerminal::DeadlineExceeded { .. } => {
            return Err(MediaImplementationError::DeadlineExceeded);
        }
        SupervisedProcessTerminal::Exited(0) => {}
        SupervisedProcessTerminal::Exited(_)
        | SupervisedProcessTerminal::Signaled
        | SupervisedProcessTerminal::ChildProcessLimitExceeded
        | SupervisedProcessTerminal::ThreadLimitExceeded
        | SupervisedProcessTerminal::DescriptorLimitExceeded => {
            return Err(MediaImplementationError::ProcessFailed);
        }
    }
    if result.stdout.len() != raw.len() || result.stdout.len() % 2 != 0 {
        return Err(MediaImplementationError::MalformedInput);
    }
    let samples = result
        .stdout
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect();
    PcmChunk::new(
        input.start_frame,
        input.sample_rate_hz,
        input.layout,
        input.discontinuity,
        samples,
    )
    .map(Some)
    .map_err(|_| MediaImplementationError::MalformedInput)
}

#[cfg(target_arch = "wasm32")]
pub fn execute_process_gain(
    _observed: &ObservedMediaArtifact,
    _tick: u64,
    _input: &PcmChunk,
    _cancellation: (),
) -> Result<Option<PcmChunk>, MediaImplementationError> {
    Err(MediaImplementationError::UnsupportedTarget)
}

#[cfg(not(target_arch = "wasm32"))]
fn process_argv(implementation: MediaImplementation) -> Vec<OsString> {
    let arguments: &[&str] = match implementation {
        MediaImplementation::FfmpegProcess => &[
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostats",
            "-threads",
            "1",
            "-filter_threads",
            "1",
            "-protocol_whitelist",
            "pipe",
            "-f",
            "s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-i",
            "pipe:0",
            "-filter:a",
            "volume=0.5",
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "pipe:1",
        ],
        MediaImplementation::SoxProcess => &[
            "-q",
            "--no-dither",
            "-t",
            "raw",
            "-e",
            "signed-integer",
            "-b",
            "16",
            "-L",
            "-r",
            "48000",
            "-c",
            "2",
            "-",
            "-t",
            "raw",
            "-e",
            "signed-integer",
            "-b",
            "16",
            "-L",
            "-r",
            "48000",
            "-c",
            "2",
            "-",
            "vol",
            "0.5",
        ],
        MediaImplementation::BrowserWasmLinked => &[],
    };
    arguments.iter().map(OsString::from).collect()
}

fn validate_chunk(chunk: &PcmChunk) -> Result<(), MediaImplementationError> {
    if chunk.sample_rate_hz != 48_000
        || chunk.layout != ChannelLayout::StereoLr
        || chunk.discontinuity
        || chunk.frames() == 0
        || chunk.frames() > MAXIMUM_PCM_FRAMES
    {
        Err(MediaImplementationError::UnsupportedProfile)
    } else {
        Ok(())
    }
}

fn pcm_value(chunk: &PcmChunk) -> Result<Value, RuntimeError> {
    Ok(Value {
        value_type: AUDIO_GAIN_CONTRACT.inputs[0].value_type,
        bytes: encode_pcm_chunk(chunk).map_err(audio_error)?,
    })
}

fn ffmpeg_handler() -> Box<dyn Handler> {
    Box::new(ProcessGainHandler {
        implementation: MediaImplementation::FfmpegProcess,
        binding: None,
    })
}

fn sox_handler() -> Box<dyn Handler> {
    Box::new(ProcessGainHandler {
        implementation: MediaImplementation::SoxProcess,
        binding: None,
    })
}

fn browser_handler() -> Box<dyn Handler> {
    Box::new(BrowserGainHandler)
}

fn runtime_error(error: MediaImplementationError) -> RuntimeError {
    RuntimeError::new(
        error.code(),
        format!("media implementation failed: {error:?}"),
    )
}

fn audio_error(error: AudioProcessingReason) -> RuntimeError {
    RuntimeError::new(error.code(), format!("media value failed: {error:?}"))
}

impl From<MediaImplementationError> for RuntimeError {
    fn from(error: MediaImplementationError) -> Self {
        runtime_error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn process_arguments_are_fixed_pipe_only_and_never_source_derived() {
        for implementation in [
            MediaImplementation::FfmpegProcess,
            MediaImplementation::SoxProcess,
        ] {
            let rendered = process_argv(implementation)
                .iter()
                .map(|value| value.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!rendered.contains("http:"));
            assert!(!rendered.contains("https:"));
            assert!(!rendered.contains("$"));
            assert!(!rendered.contains(";"));
            assert!(!rendered.contains("*"));
        }
        let ffmpeg = process_argv(MediaImplementation::FfmpegProcess);
        let whitelist = ffmpeg
            .iter()
            .position(|argument| argument == "-protocol_whitelist")
            .unwrap();
        assert_eq!(ffmpeg[whitelist + 1], "pipe");
    }

    #[test]
    fn observation_summary_redacts_absolute_paths_and_runtime_limits_are_finite() {
        let observation = ObservedMediaArtifact {
            implementation: MediaImplementation::FfmpegProcess,
            digest: ArtifactDigest::from_bytes([9; 32]),
            byte_size: 1,
            executable: Some(PathBuf::from("/secret/provider/bin/ffmpeg")),
            version: "ffmpeg version bounded".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
            limits: MediaImplementationLimits::process(),
        };
        let summary = observation.redacted_summary();
        assert!(summary.contains("artifact=ffmpeg"));
        assert!(!summary.contains("/secret/provider"));
        assert_eq!(observation.limits.maximum_processes, 1);
        assert_eq!(observation.limits.maximum_child_processes, 0);
        assert_eq!(observation.limits.maximum_temp_bytes, 0);
        assert!(observation.limits.maximum_evidence_events > 0);
        assert_eq!(observation.limits.maximum_pending_callbacks, 0);
        assert_eq!(observation.limits.maximum_foreign_queue, 0);
        let browser = MediaImplementationLimits::browser();
        assert_eq!(browser.maximum_pending_callbacks, 1);
        assert_eq!(browser.maximum_foreign_queue, 1);
    }
}
