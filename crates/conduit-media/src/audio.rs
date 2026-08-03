//! Exact bounded PCM processing contracts and deterministic reference providers.
//!
//! The semantic boundary owns alignment, numeric, layout, timing, retained-state,
//! pressure, and terminal policy. Sample arithmetic stays in the provider below.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, ExactHostedServiceBinding, Handler, HostedServiceInterest,
    HostedServiceStep, HostedServiceStepContext, Registry, RegistryError, ResolutionError, RunIo,
    RuntimeError, Value,
};

use crate::{AUDIO_FRAME_TYPE, CONTROL_TYPE};

pub const MAXIMUM_AUDIO_INPUTS: usize = 2;
pub const MAXIMUM_PCM_CHANNELS: usize = 2;
pub const MAXIMUM_PCM_FRAMES: usize = 32;
pub const MAXIMUM_PCM_SAMPLES: usize = MAXIMUM_PCM_CHANNELS * MAXIMUM_PCM_FRAMES;
pub const MAXIMUM_AUDIO_WORK: usize = 256;
pub const MAXIMUM_AUTOMATION_POINTS: usize = 2;
pub const MAXIMUM_METER_SIDE_OUTPUTS: usize = 1;
pub const MAXIMUM_AUDIO_VALUE_BYTES: usize = 24 + MAXIMUM_PCM_SAMPLES * 2;

pub const REFERENCE_NUMERIC_PROFILE: &str =
    "pcm-s16-q15-round-nearest-away-saturate-no-nan-no-denormal-bit-exact";
pub const REFERENCE_PROVIDER_ID: &str = "conduit.media/audio-processing-reference";
pub const OPTIMIZED_PROVIDER_PROFILE_ID: &str =
    "conduit.media/audio-processing-optimized-s16-exact";

const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelLayout {
    Mono,
    StereoLr,
    StereoRl,
}

impl ChannelLayout {
    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::StereoLr | Self::StereoRl => 2,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mono => "mono-center",
            Self::StereoLr => "stereo-lr",
            Self::StereoRl => "stereo-rl",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Mono => 1,
            Self::StereoLr => 2,
            Self::StereoRl => 3,
        }
    }

    const fn from_tag(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Mono),
            2 => Some(Self::StereoLr),
            3 => Some(Self::StereoRl),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcmChunk {
    pub start_frame: u64,
    pub sample_rate_hz: u32,
    pub layout: ChannelLayout,
    pub discontinuity: bool,
    pub samples: Vec<i16>,
}

impl PcmChunk {
    pub fn new(
        start_frame: u64,
        sample_rate_hz: u32,
        layout: ChannelLayout,
        discontinuity: bool,
        samples: Vec<i16>,
    ) -> Result<Self, AudioProcessingReason> {
        let value = Self {
            start_frame,
            sample_rate_hz,
            layout,
            discontinuity,
            samples,
        };
        value.validate()?;
        Ok(value)
    }

    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / self.layout.channels()
    }

    pub fn validate(&self) -> Result<(), AudioProcessingReason> {
        let channels = self.layout.channels();
        if self.sample_rate_hz == 0
            || self.samples.is_empty()
            || self.samples.len() > MAXIMUM_PCM_SAMPLES
            || self.samples.len() % channels != 0
            || self.frames() > MAXIMUM_PCM_FRAMES
        {
            return Err(AudioProcessingReason::Bounds);
        }
        self.start_frame
            .checked_add(self.frames() as u64)
            .ok_or(AudioProcessingReason::Timestamp)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioProcessingReason {
    Representation,
    Bounds,
    Timestamp,
    Alignment,
    MissingOrLateInput,
    UnsupportedFormat,
    UnsupportedRate,
    LayoutMismatch,
    MatrixMismatch,
    NumericProfile,
    Discontinuity,
    Work,
    Pressure,
    Cancelled,
    RetainedAtTerminal,
}

impl AudioProcessingReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Representation => "CND-AUDIO-001",
            Self::Bounds | Self::Work => "CND-AUDIO-002",
            Self::Timestamp | Self::Alignment => "CND-AUDIO-003",
            Self::MissingOrLateInput => "CND-AUDIO-004",
            Self::UnsupportedFormat | Self::UnsupportedRate => "CND-AUDIO-005",
            Self::LayoutMismatch | Self::MatrixMismatch => "CND-AUDIO-006",
            Self::NumericProfile => "CND-AUDIO-007",
            Self::Discontinuity => "CND-AUDIO-008",
            Self::Pressure => "CND-AUDIO-009",
            Self::Cancelled => "CND-AUDIO-010",
            Self::RetainedAtTerminal => "CND-AUDIO-011",
        }
    }
}

fn saturating_q15(sample: i64, gain_q15: u32) -> i16 {
    let product = sample.saturating_mul(i64::from(gain_q15));
    let rounded = if product >= 0 {
        product.saturating_add(1 << 14)
    } else {
        product.saturating_sub(1 << 14)
    } / (1 << 15);
    i16::try_from(rounded).unwrap_or(if rounded < 0 { i16::MIN } else { i16::MAX })
}

pub fn mix_pcm(
    left: &PcmChunk,
    right: &PcmChunk,
    left_gain_q15: u32,
    right_gain_q15: u32,
) -> Result<PcmChunk, AudioProcessingReason> {
    left.validate()?;
    right.validate()?;
    if left.start_frame != right.start_frame || left.frames() != right.frames() {
        return Err(AudioProcessingReason::Alignment);
    }
    if left.sample_rate_hz != right.sample_rate_hz {
        return Err(AudioProcessingReason::UnsupportedRate);
    }
    if left.layout != right.layout {
        return Err(AudioProcessingReason::LayoutMismatch);
    }
    if left.discontinuity != right.discontinuity {
        return Err(AudioProcessingReason::Discontinuity);
    }
    if left.samples.len() > MAXIMUM_AUDIO_WORK || right.samples.len() > MAXIMUM_AUDIO_WORK {
        return Err(AudioProcessingReason::Work);
    }
    let samples = left
        .samples
        .iter()
        .zip(&right.samples)
        .map(|(left, right)| {
            let left = i64::from(saturating_q15(i64::from(*left), left_gain_q15));
            let right = i64::from(saturating_q15(i64::from(*right), right_gain_q15));
            i16::try_from(left.saturating_add(right)).unwrap_or(if left + right < 0 {
                i16::MIN
            } else {
                i16::MAX
            })
        })
        .collect();
    PcmChunk::new(
        left.start_frame,
        left.sample_rate_hz,
        left.layout,
        left.discontinuity,
        samples,
    )
}

pub fn gain_pcm(
    input: &PcmChunk,
    start_gain_q15: u32,
    end_gain_q15: u32,
    ramp_start_frame: u64,
    ramp_end_frame: u64,
) -> Result<PcmChunk, AudioProcessingReason> {
    input.validate()?;
    if ramp_end_frame < ramp_start_frame || start_gain_q15 > 131_072 || end_gain_q15 > 131_072 {
        return Err(AudioProcessingReason::NumericProfile);
    }
    let channels = input.layout.channels();
    let span = ramp_end_frame.saturating_sub(ramp_start_frame);
    let mut samples = Vec::with_capacity(input.samples.len());
    for frame in 0..input.frames() {
        let absolute = input.start_frame + frame as u64;
        let gain = if absolute <= ramp_start_frame || span == 0 {
            start_gain_q15
        } else if absolute >= ramp_end_frame {
            end_gain_q15
        } else {
            let elapsed = absolute - ramp_start_frame;
            let start = i128::from(start_gain_q15);
            let delta = i128::from(end_gain_q15) - start;
            u32::try_from(start + delta * i128::from(elapsed) / i128::from(span))
                .map_err(|_| AudioProcessingReason::NumericProfile)?
        };
        for channel in 0..channels {
            samples.push(saturating_q15(
                i64::from(input.samples[frame * channels + channel]),
                gain,
            ));
        }
    }
    PcmChunk::new(
        input.start_frame,
        input.sample_rate_hz,
        input.layout,
        input.discontinuity,
        samples,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelMatrix {
    StereoLrToMonoAverage,
    MonoToStereoCopy,
    StereoLrIdentity,
    StereoLrToStereoRlSwap,
}

impl ChannelMatrix {
    #[must_use]
    pub const fn identity(self) -> &'static str {
        match self {
            Self::StereoLrToMonoAverage => "mono=(16384*L+16384*R)/32768",
            Self::MonoToStereoCopy => "L=32768*M/32768;R=32768*M/32768",
            Self::StereoLrIdentity => "L=32768*L/32768;R=32768*R/32768",
            Self::StereoLrToStereoRlSwap => "R=32768*R/32768;L=32768*L/32768",
        }
    }
}

pub fn channel_map_pcm(
    input: &PcmChunk,
    matrix: ChannelMatrix,
) -> Result<PcmChunk, AudioProcessingReason> {
    input.validate()?;
    let (required, output_layout) = match matrix {
        ChannelMatrix::StereoLrToMonoAverage => (ChannelLayout::StereoLr, ChannelLayout::Mono),
        ChannelMatrix::MonoToStereoCopy => (ChannelLayout::Mono, ChannelLayout::StereoLr),
        ChannelMatrix::StereoLrIdentity => (ChannelLayout::StereoLr, ChannelLayout::StereoLr),
        ChannelMatrix::StereoLrToStereoRlSwap => (ChannelLayout::StereoLr, ChannelLayout::StereoRl),
    };
    if input.layout != required {
        return Err(AudioProcessingReason::LayoutMismatch);
    }
    let mut samples = Vec::with_capacity(input.frames() * output_layout.channels());
    match matrix {
        ChannelMatrix::StereoLrToMonoAverage => {
            for pair in input.samples.chunks_exact(2) {
                samples.push(saturating_q15(
                    i64::from(pair[0]) + i64::from(pair[1]),
                    16_384,
                ));
            }
        }
        ChannelMatrix::MonoToStereoCopy => {
            for sample in &input.samples {
                samples.extend_from_slice(&[*sample, *sample]);
            }
        }
        ChannelMatrix::StereoLrIdentity => samples.extend_from_slice(&input.samples),
        ChannelMatrix::StereoLrToStereoRlSwap => {
            for pair in input.samples.chunks_exact(2) {
                samples.extend_from_slice(&[pair[1], pair[0]]);
            }
        }
    }
    PcmChunk::new(
        input.start_frame,
        input.sample_rate_hz,
        output_layout,
        input.discontinuity,
        samples,
    )
}

pub fn resample_pcm(
    input: &PcmChunk,
    output_rate_hz: u32,
) -> Result<PcmChunk, AudioProcessingReason> {
    input.validate()?;
    if input.discontinuity {
        return Err(AudioProcessingReason::Discontinuity);
    }
    let channels = input.layout.channels();
    let (output_start, samples) = if output_rate_hz == input.sample_rate_hz {
        (input.start_frame, input.samples.clone())
    } else if input.sample_rate_hz == output_rate_hz.saturating_mul(2) {
        let mut samples = Vec::new();
        let first = if input.start_frame % 2 == 0 { 0 } else { 1 };
        for frame in (first..input.frames()).step_by(2) {
            samples.extend_from_slice(&input.samples[frame * channels..(frame + 1) * channels]);
        }
        ((input.start_frame + first as u64) / 2, samples)
    } else if output_rate_hz == input.sample_rate_hz.saturating_mul(2) {
        let mut samples = Vec::with_capacity(input.samples.len() * 2);
        for frame in input.samples.chunks_exact(channels) {
            samples.extend_from_slice(frame);
            samples.extend_from_slice(frame);
        }
        (input.start_frame.saturating_mul(2), samples)
    } else {
        return Err(AudioProcessingReason::UnsupportedRate);
    };
    if samples.is_empty() || samples.len() > MAXIMUM_PCM_SAMPLES {
        return Err(AudioProcessingReason::Bounds);
    }
    PcmChunk::new(output_start, output_rate_hz, input.layout, false, samples)
}

pub fn trim_pcm(
    input: &PcmChunk,
    interval_start_frame: u64,
    interval_end_frame: Option<u64>,
    fade_in_frames: u64,
    fade_out_frames: u64,
) -> Result<PcmChunk, AudioProcessingReason> {
    input.validate()?;
    if interval_end_frame.is_some_and(|end| end < interval_start_frame) {
        return Err(AudioProcessingReason::Timestamp);
    }
    let channels = input.layout.channels();
    let mut first = None;
    let mut samples = Vec::new();
    for frame in 0..input.frames() {
        let absolute = input.start_frame + frame as u64;
        if absolute < interval_start_frame || interval_end_frame.is_some_and(|end| absolute >= end)
        {
            continue;
        }
        first.get_or_insert(absolute);
        let mut gain = 32_768_u32;
        if fade_in_frames > 0 && absolute < interval_start_frame.saturating_add(fade_in_frames) {
            gain = u32::try_from(
                (absolute - interval_start_frame).saturating_mul(32_768) / fade_in_frames,
            )
            .unwrap_or(32_768);
        }
        if let Some(end) = interval_end_frame {
            if fade_out_frames > 0 && absolute >= end.saturating_sub(fade_out_frames) {
                let remaining = end.saturating_sub(absolute + 1);
                gain = gain.min(
                    u32::try_from(remaining.saturating_mul(32_768) / fade_out_frames)
                        .unwrap_or(32_768),
                );
            }
        }
        for channel in 0..channels {
            samples.push(saturating_q15(
                i64::from(input.samples[frame * channels + channel]),
                gain,
            ));
        }
    }
    let start_frame = first.ok_or(AudioProcessingReason::Bounds)?;
    PcmChunk::new(
        start_frame,
        input.sample_rate_hz,
        input.layout,
        input.discontinuity && start_frame == input.start_frame,
        samples,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeterReading {
    pub start_frame: u64,
    pub frames: usize,
    pub peak: u16,
    pub rms: u16,
}

fn integer_sqrt(value: u128) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut x = value;
    let mut y = x / 2 + x % 2;
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }
    u64::try_from(x).unwrap_or(u64::MAX)
}

pub fn meter_pcm(input: &PcmChunk) -> Result<MeterReading, AudioProcessingReason> {
    input.validate()?;
    let mut peak = 0_u32;
    let mut squares = 0_u128;
    for sample in &input.samples {
        let magnitude = i32::from(*sample).unsigned_abs();
        peak = peak.max(magnitude);
        squares = squares.saturating_add(u128::from(magnitude) * u128::from(magnitude));
    }
    let mean = squares / input.samples.len() as u128;
    Ok(MeterReading {
        start_frame: input.start_frame,
        frames: input.frames(),
        peak: u16::try_from(peak.min(u32::from(u16::MAX))).unwrap_or(u16::MAX),
        rms: u16::try_from(integer_sqrt(mean).min(u64::from(u16::MAX))).unwrap_or(u16::MAX),
    })
}

pub fn encode_pcm_chunk(chunk: &PcmChunk) -> Result<Vec<u8>, AudioProcessingReason> {
    chunk.validate()?;
    let mut bytes = Vec::with_capacity(24 + chunk.samples.len() * 2);
    bytes.extend_from_slice(b"CAP0");
    bytes.push(0);
    bytes.push(chunk.layout.tag());
    bytes.push(u8::from(chunk.discontinuity));
    bytes.push(0);
    bytes.extend_from_slice(&chunk.start_frame.to_le_bytes());
    bytes.extend_from_slice(&chunk.sample_rate_hz.to_le_bytes());
    bytes.extend_from_slice(&(chunk.frames() as u16).to_le_bytes());
    bytes.extend_from_slice(&(chunk.layout.channels() as u16).to_le_bytes());
    for sample in &chunk.samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

pub fn decode_pcm_chunk(value: &Value) -> Result<PcmChunk, AudioProcessingReason> {
    if value.value_type != AUDIO_FRAME_TYPE {
        return Err(AudioProcessingReason::UnsupportedFormat);
    }
    if value.bytes.starts_with(b"CMA0") && value.bytes.len() <= 64 {
        let silence = value.bytes.get(4) == Some(&b'S');
        let mut samples = Vec::with_capacity(32);
        for frame in 0..16 {
            let sample = if silence {
                0
            } else if frame % 2 == 0 {
                12_000
            } else {
                -12_000
            };
            samples.extend_from_slice(&[sample, sample]);
        }
        return PcmChunk::new(0, 48_000, ChannelLayout::StereoLr, false, samples);
    }
    if value.bytes.len() < 24
        || value.bytes.len() > MAXIMUM_AUDIO_VALUE_BYTES
        || !value.bytes.starts_with(b"CAP0")
        || value.bytes[4] != 0
        || value.bytes[6] > 1
        || value.bytes[7] != 0
    {
        return Err(AudioProcessingReason::Representation);
    }
    let layout =
        ChannelLayout::from_tag(value.bytes[5]).ok_or(AudioProcessingReason::Representation)?;
    let start_frame = u64::from_le_bytes(
        value.bytes[8..16]
            .try_into()
            .map_err(|_| AudioProcessingReason::Representation)?,
    );
    let sample_rate_hz = u32::from_le_bytes(
        value.bytes[16..20]
            .try_into()
            .map_err(|_| AudioProcessingReason::Representation)?,
    );
    let frames = usize::from(u16::from_le_bytes(
        value.bytes[20..22]
            .try_into()
            .map_err(|_| AudioProcessingReason::Representation)?,
    ));
    let channels = usize::from(u16::from_le_bytes(
        value.bytes[22..24]
            .try_into()
            .map_err(|_| AudioProcessingReason::Representation)?,
    ));
    if channels != layout.channels() || value.bytes.len() != 24 + frames * channels * 2 {
        return Err(AudioProcessingReason::Representation);
    }
    let samples = value.bytes[24..]
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect();
    PcmChunk::new(
        start_frame,
        sample_rate_hz,
        layout,
        value.bytes[6] == 1,
        samples,
    )
}

fn pcm_value(chunk: &PcmChunk) -> Result<Value, RuntimeError> {
    Ok(Value {
        value_type: AUDIO_FRAME_TYPE,
        bytes: encode_pcm_chunk(chunk).map_err(runtime_reason)?,
    })
}

const fn field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

const fn plan_field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Plan,
    }
}

const fn secret_plan_field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Secret,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Plan,
    }
}

const fn stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    connections: ConnectionCardinality,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const AUDIO_INPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Input,
    AUDIO_FRAME_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const AUDIO_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Output,
    AUDIO_FRAME_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const MIX_INPUTS: [PortContract<'static>; 2] = [
    stream_port(
        "left",
        Direction::Input,
        AUDIO_FRAME_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    stream_port(
        "right",
        Direction::Input,
        AUDIO_FRAME_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const TEE_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port(
        "left",
        Direction::Output,
        AUDIO_FRAME_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    stream_port(
        "right",
        Direction::Output,
        AUDIO_FRAME_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const METER_OUTPUTS: [PortContract<'static>; 1] = [stream_port(
    "level",
    Direction::Output,
    TEXT_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const CONTROL_INPUT: [PortContract<'static>; 1] = [stream_port(
    "level",
    Direction::Input,
    CONTROL_TYPE,
    ConnectionCardinality::ExactlyOne,
)];

const MIX_FIELDS: [ConfigFieldContract<'static>; 14] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("alignment", TEXT_TYPE),
    field("missing_late", TEXT_TYPE),
    field("left_gain_q15", U64_TYPE),
    field("right_gain_q15", U64_TYPE),
    field("headroom", TEXT_TYPE),
    field("clipping", TEXT_TYPE),
    field("output_layout", TEXT_TYPE),
    field("latency_frames", U64_TYPE),
    field("terminal_drain", TEXT_TYPE),
    field("maximum_inputs", U64_TYPE),
    field("maximum_frames", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const TEE_FIELDS: [ConfigFieldContract<'static>; 3] = [
    field("lifecycle", TEXT_TYPE),
    field("mode", TEXT_TYPE),
    field("maximum_frames", U64_TYPE),
];
const GAIN_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("curve", TEXT_TYPE),
    field("start_gain_q15", U64_TYPE),
    field("end_gain_q15", U64_TYPE),
    field("ramp_start_frame", U64_TYPE),
    field("ramp_end_frame", U64_TYPE),
    field("discontinuity", TEXT_TYPE),
    field("maximum_automation_points", U64_TYPE),
    field("maximum_retained_samples", U64_TYPE),
    field("maximum_frames", U64_TYPE),
];
const CHANNEL_FIELDS: [ConfigFieldContract<'static>; 7] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("input_layout", TEXT_TYPE),
    field("output_layout", TEXT_TYPE),
    field("matrix_q15", TEXT_TYPE),
    field("maximum_channels", U64_TYPE),
    field("maximum_frames", U64_TYPE),
];
const RESAMPLE_FIELDS: [ConfigFieldContract<'static>; 12] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("input_rate_hz", U64_TYPE),
    field("output_rate_hz", U64_TYPE),
    field("quality_profile", TEXT_TYPE),
    field("group_delay_frames", U64_TYPE),
    field("timestamp_mapping", TEXT_TYPE),
    field("drift", TEXT_TYPE),
    field("flush", TEXT_TYPE),
    field("maximum_history_frames", U64_TYPE),
    field("maximum_frames", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const TRIM_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("interval_basis", TEXT_TYPE),
    field("start_frame", U64_TYPE),
    field("end_frame", U64_TYPE),
    field("open_ended", TEXT_TYPE),
    field("boundary_rounding", TEXT_TYPE),
    field("discontinuity", TEXT_TYPE),
    field("fade_in_frames", U64_TYPE),
    field("fade_out_frames", U64_TYPE),
    field("maximum_frames", U64_TYPE),
];
const METER_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("window_basis", TEXT_TYPE),
    field("window_frames", U64_TYPE),
    field("peak", TEXT_TYPE),
    field("rms", TEXT_TYPE),
    field("cadence_frames", U64_TYPE),
    field("latency_frames", U64_TYPE),
    field("maximum_retained_samples", U64_TYPE),
    field("maximum_side_outputs", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const FROM_CONTROL_FIELDS: [ConfigFieldContract<'static>; 6] = [
    field("lifecycle", TEXT_TYPE),
    field("numeric_profile", TEXT_TYPE),
    field("sample_rate_hz", U64_TYPE),
    field("layout", TEXT_TYPE),
    field("frames_per_control", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const CAPTURE_FIELDS: [ConfigFieldContract<'static>; 39] = [
    secret_plan_field("device_resource", TEXT_TYPE),
    plan_field("device_label", TEXT_TYPE),
    plan_field("provider_observation", TEXT_TYPE),
    plan_field("observation_generation", U64_TYPE),
    plan_field("observation_valid_until_tick", U64_TYPE),
    plan_field("backend_identity", TEXT_TYPE),
    field("sample_format", TEXT_TYPE),
    field("sample_rate_hz", U64_TYPE),
    field("layout", TEXT_TYPE),
    plan_field("sample_clock", TEXT_TYPE),
    field("clock_correlation", TEXT_TYPE),
    field("requested_period_frames", U64_TYPE),
    plan_field("admitted_period_frames", U64_TYPE),
    field("requested_buffer_frames", U64_TYPE),
    plan_field("admitted_buffer_frames", U64_TYPE),
    field("requested_latency_frames", U64_TYPE),
    plan_field("admitted_latency_frames", U64_TYPE),
    plan_field("latency_classification", TEXT_TYPE),
    plan_field("sharing_mode", TEXT_TYPE),
    plan_field("maximum_concurrent_streams", U64_TYPE),
    plan_field("workload_class", TEXT_TYPE),
    field("lifecycle", TEXT_TYPE),
    field("underrun", TEXT_TYPE),
    field("overrun", TEXT_TYPE),
    field("drift", TEXT_TYPE),
    field("discontinuity", TEXT_TYPE),
    field("provider_loss", TEXT_TYPE),
    field("cancellation", TEXT_TYPE),
    field("drain", TEXT_TYPE),
    field("commit_point", TEXT_TYPE),
    secret_plan_field("device_grant", TEXT_TYPE),
    plan_field("lease_ticks", U64_TYPE),
    plan_field("revocation_grace_ticks", U64_TYPE),
    plan_field("cleanup_ticks", U64_TYPE),
    field("sensitivity", TEXT_TYPE),
    field("maximum_frames_per_step", U64_TYPE),
    plan_field("maximum_host_queue_frames", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];
const PLAYBACK_FIELDS: [ConfigFieldContract<'static>; 39] = [
    secret_plan_field("device_resource", TEXT_TYPE),
    plan_field("device_label", TEXT_TYPE),
    plan_field("provider_observation", TEXT_TYPE),
    plan_field("observation_generation", U64_TYPE),
    plan_field("observation_valid_until_tick", U64_TYPE),
    plan_field("backend_identity", TEXT_TYPE),
    field("sample_format", TEXT_TYPE),
    field("sample_rate_hz", U64_TYPE),
    field("layout", TEXT_TYPE),
    plan_field("sample_clock", TEXT_TYPE),
    field("clock_correlation", TEXT_TYPE),
    field("requested_period_frames", U64_TYPE),
    plan_field("admitted_period_frames", U64_TYPE),
    field("requested_buffer_frames", U64_TYPE),
    plan_field("admitted_buffer_frames", U64_TYPE),
    field("requested_latency_frames", U64_TYPE),
    plan_field("admitted_latency_frames", U64_TYPE),
    plan_field("latency_classification", TEXT_TYPE),
    plan_field("sharing_mode", TEXT_TYPE),
    plan_field("maximum_concurrent_streams", U64_TYPE),
    plan_field("workload_class", TEXT_TYPE),
    field("lifecycle", TEXT_TYPE),
    field("underrun", TEXT_TYPE),
    field("overrun", TEXT_TYPE),
    field("drift", TEXT_TYPE),
    field("discontinuity", TEXT_TYPE),
    field("provider_loss", TEXT_TYPE),
    field("cancellation", TEXT_TYPE),
    field("drain", TEXT_TYPE),
    field("commit_point", TEXT_TYPE),
    secret_plan_field("device_grant", TEXT_TYPE),
    plan_field("lease_ticks", U64_TYPE),
    plan_field("revocation_grace_ticks", U64_TYPE),
    plan_field("cleanup_ticks", U64_TYPE),
    field("sensitivity", TEXT_TYPE),
    field("maximum_frames_per_step", U64_TYPE),
    plan_field("maximum_host_queue_frames", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];

pub const AUDIO_MIX_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/mix"),
    config: ConfigContract {
        fields: &MIX_FIELDS,
    },
    inputs: &MIX_INPUTS,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_TEE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/tee"),
    config: ConfigContract {
        fields: &TEE_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &TEE_OUTPUTS,
};
pub const AUDIO_GAIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/gain"),
    config: ConfigContract {
        fields: &GAIN_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_CHANNEL_MAP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/channel-map"),
    config: ConfigContract {
        fields: &CHANNEL_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_RESAMPLE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/resample"),
    config: ConfigContract {
        fields: &RESAMPLE_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_TRIM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/trim"),
    config: ConfigContract {
        fields: &TRIM_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_METER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/meter"),
    config: ConfigContract {
        fields: &METER_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &METER_OUTPUTS,
};
pub const AUDIO_FROM_CONTROL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/from-control"),
    config: ConfigContract {
        fields: &FROM_CONTROL_FIELDS,
    },
    inputs: &CONTROL_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_CAPTURE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/capture"),
    config: ConfigContract {
        fields: &CAPTURE_FIELDS,
    },
    inputs: &[],
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_PLAYBACK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/playback"),
    config: ConfigContract {
        fields: &PLAYBACK_FIELDS,
    },
    inputs: &AUDIO_INPUT,
    outputs: &[],
};

pub const AUDIO_PROCESSING_CONTRACTS: [&NodeContract<'static>; 10] = [
    &AUDIO_TEE_CONTRACT,
    &AUDIO_MIX_CONTRACT,
    &AUDIO_GAIN_CONTRACT,
    &AUDIO_CHANNEL_MAP_CONTRACT,
    &AUDIO_RESAMPLE_CONTRACT,
    &AUDIO_TRIM_CONTRACT,
    &AUDIO_METER_CONTRACT,
    &AUDIO_FROM_CONTROL_CONTRACT,
    &AUDIO_CAPTURE_CONTRACT,
    &AUDIO_PLAYBACK_CONTRACT,
];

fn integer(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    let Some(SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-AUDIO-002",
            format!("audio configuration `{key}` must be an integer"),
        ));
    };
    u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-AUDIO-002",
            format!("audio configuration `{key}` must be nonnegative"),
        )
    })
}

fn require_text(node: &Node, key: &str, expected: &str) -> Result<(), ResolutionError> {
    if node.config(key) == Some(expected) {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-AUDIO-005",
            format!("audio configuration `{key}` must be `{expected}`"),
        ))
    }
}

fn require_secret(node: &Node, key: &str, expected: &str) -> Result<(), ResolutionError> {
    if matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    ) {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-AUDIO-005",
            format!("audio configuration `{key}` requires exact protected binding `{expected}`"),
        ))
    }
}

fn validate_lifecycle_and_profile(node: &Node) -> Result<(), ResolutionError> {
    if !matches!(node.config("lifecycle"), Some("finite") | Some("standing")) {
        return Err(ResolutionError::new(
            "CND-AUDIO-004",
            "audio lifecycle must be finite or standing",
        ));
    }
    require_text(node, "numeric_profile", REFERENCE_NUMERIC_PROFILE)
}

fn bound(node: &Node, key: &str, maximum: u64) -> Result<u64, ResolutionError> {
    let value = integer(node, key)?;
    if value == 0 || value > maximum {
        Err(ResolutionError::new(
            "CND-AUDIO-002",
            format!("audio bound `{key}` must be within 1..={maximum}"),
        ))
    } else {
        Ok(value)
    }
}

fn validate_mix(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "alignment", "exact-start-rate-layout-frames")?;
    require_text(node, "missing_late", "wait-then-fail-terminal")?;
    require_text(node, "headroom", "none")?;
    require_text(node, "clipping", "saturate-s16")?;
    require_text(node, "output_layout", "same-as-aligned-inputs")?;
    require_text(node, "terminal_drain", "no-retained-samples")?;
    if node.config.len() != MIX_FIELDS.len()
        || integer(node, "maximum_inputs")? != MAXIMUM_AUDIO_INPUTS as u64
        || integer(node, "latency_frames")? != 0
        || integer(node, "left_gain_q15")? > 131_072
        || integer(node, "right_gain_q15")? > 131_072
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-002",
            "mix requires exactly two bounded inputs and Q15 gains at or below 4x",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    Ok(())
}

fn validate_tee(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() != TEE_FIELDS.len()
        || !matches!(node.config("lifecycle"), Some("finite") | Some("standing"))
        || node.config("mode") != Some("coupled")
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-002",
            "audio tee requires explicit coupled delivery and lifecycle",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    Ok(())
}

fn validate_gain(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "curve", "linear-q15-absolute-frame")?;
    require_text(node, "discontinuity", "absolute-timeline")?;
    if node.config.len() != GAIN_FIELDS.len()
        || integer(node, "start_gain_q15")? > 131_072
        || integer(node, "end_gain_q15")? > 131_072
        || integer(node, "ramp_end_frame")? < integer(node, "ramp_start_frame")?
        || integer(node, "maximum_automation_points")? != MAXIMUM_AUTOMATION_POINTS as u64
        || integer(node, "maximum_retained_samples")? != 0
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-007",
            "gain requires one exact two-point bounded linear Q15 ramp",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    Ok(())
}

fn parse_matrix(node: &Node) -> Result<ChannelMatrix, ResolutionError> {
    let matrix = match node.config("matrix_q15") {
        Some("mono=(16384*L+16384*R)/32768") => ChannelMatrix::StereoLrToMonoAverage,
        Some("L=32768*M/32768;R=32768*M/32768") => ChannelMatrix::MonoToStereoCopy,
        Some("L=32768*L/32768;R=32768*R/32768") => ChannelMatrix::StereoLrIdentity,
        Some("R=32768*R/32768;L=32768*L/32768") => ChannelMatrix::StereoLrToStereoRlSwap,
        _ => {
            return Err(ResolutionError::new(
                "CND-AUDIO-006",
                "channel map matrix is not an exact supported Q15 matrix",
            ));
        }
    };
    if node.config("input_layout")
        != Some(match matrix {
            ChannelMatrix::MonoToStereoCopy => "mono-center",
            _ => "stereo-lr",
        })
        || node.config("output_layout")
            != Some(match matrix {
                ChannelMatrix::StereoLrToMonoAverage => "mono-center",
                ChannelMatrix::StereoLrToStereoRlSwap => "stereo-rl",
                _ => "stereo-lr",
            })
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-006",
            "channel names/order and the explicit matrix disagree",
        ));
    }
    Ok(matrix)
}

fn validate_channel_map(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    parse_matrix(node)?;
    if node.config.len() != CHANNEL_FIELDS.len()
        || integer(node, "maximum_channels")? != MAXIMUM_PCM_CHANNELS as u64
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-006",
            "channel map must bind exactly two-or-fewer named channels",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    Ok(())
}

fn validate_resample(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "quality_profile", "nearest-hold-bit-exact")?;
    require_text(node, "timestamp_mapping", "absolute-rational-grid")?;
    require_text(node, "drift", "reject-rate-or-discontinuity")?;
    require_text(node, "flush", "no-pending-output")?;
    let input = integer(node, "input_rate_hz")?;
    let output = integer(node, "output_rate_hz")?;
    if node.config.len() != RESAMPLE_FIELDS.len()
        || !matches!(
            (input, output),
            (48_000, 48_000 | 24_000) | (24_000, 48_000 | 24_000)
        )
        || integer(node, "group_delay_frames")? != 0
        || integer(node, "maximum_history_frames")? != 0
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-005",
            "reference resample supports exact 24/48 kHz ratios with zero delay/history",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    Ok(())
}

fn validate_trim(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "interval_basis", "absolute-input-frame-half-open")?;
    require_text(node, "boundary_rounding", "ceil-start-floor-end")?;
    require_text(node, "discontinuity", "preserve-if-first-retained")?;
    if node.config.len() != TRIM_FIELDS.len()
        || !matches!(node.config("open_ended"), Some("true") | Some("false"))
        || (node.config("open_ended") == Some("false")
            && integer(node, "end_frame")? < integer(node, "start_frame")?)
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-003",
            "trim requires one exact half-open interval",
        ));
    }
    bound(node, "maximum_frames", MAXIMUM_PCM_FRAMES as u64)?;
    Ok(())
}

fn validate_meter(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "window_basis", "input-frame-clock")?;
    require_text(node, "peak", "absolute-s16")?;
    require_text(node, "rms", "integer-sqrt-mean-square")?;
    let window = bound(node, "window_frames", MAXIMUM_PCM_FRAMES as u64)?;
    if node.config.len() != METER_FIELDS.len()
        || integer(node, "cadence_frames")? != window
        || integer(node, "latency_frames")? != 0
        || integer(node, "maximum_retained_samples")? != 0
        || integer(node, "maximum_side_outputs")? != MAXIMUM_METER_SIDE_OUTPUTS as u64
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-002",
            "meter reference window has exact cadence, zero latency/history, and one side output",
        ));
    }
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    Ok(())
}

fn validate_from_control(node: &Node) -> Result<(), ResolutionError> {
    validate_lifecycle_and_profile(node)?;
    require_text(node, "layout", "stereo-lr")?;
    if node.config.len() != FROM_CONTROL_FIELDS.len() || integer(node, "sample_rate_hz")? != 48_000
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-005",
            "control-to-PCM reference adapter supports stereo 48 kHz",
        ));
    }
    bound(node, "frames_per_control", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    Ok(())
}

fn validate_capture(node: &Node) -> Result<(), ResolutionError> {
    require_secret(
        node,
        "device_resource",
        "conduit.audio/device/virtual-capture-0",
    )?;
    require_secret(node, "device_grant", "conduit.audio/grant/virtual-capture")?;
    for (key, expected) in [
        ("device_label", "Virtual Loopback Capture"),
        (
            "provider_observation",
            "conduit.audio/observation/virtual-loopback",
        ),
        (
            "backend_identity",
            "conduit.audio/backend/deterministic-loopback",
        ),
        ("sample_format", "pcm-s16le-interleaved"),
        ("sample_clock", "conduit.clock/virtual-audio-48000"),
        ("layout", "stereo-lr"),
        ("clock_correlation", "exact"),
        ("latency_classification", "enforced"),
        ("sharing_mode", "exclusive"),
        ("workload_class", "deterministic-bounded"),
        ("lifecycle", "standing"),
        ("underrun", "not-applicable"),
        ("overrun", "fail-terminal-evidenced"),
        ("drift", "reject-evidenced"),
        ("discontinuity", "fail-terminal-evidenced"),
        ("provider_loss", "fail-terminal-evidenced"),
        (
            "cancellation",
            "before-open-after-open-running-drain-distinct",
        ),
        ("drain", "not-applicable"),
        ("commit_point", "first-sample-delivered"),
        ("sensitivity", "restricted-audio"),
    ] {
        require_text(node, key, expected)?;
    }
    if node.config.len() != CAPTURE_FIELDS.len()
        || integer(node, "observation_generation")? != 1
        || integer(node, "sample_rate_hz")? != 48_000
        || integer(node, "requested_period_frames")? != 8
        || integer(node, "admitted_period_frames")? != 8
        || integer(node, "requested_buffer_frames")? != 8
        || integer(node, "admitted_buffer_frames")? != 8
        || integer(node, "requested_latency_frames")? != 8
        || integer(node, "admitted_latency_frames")? != 8
        || integer(node, "maximum_concurrent_streams")? != 1
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-005",
            "the deterministic virtual capture profile requires one exact admitted stereo 48 kHz stream",
        ));
    }
    bound(node, "observation_valid_until_tick", 1_000_000)?;
    bound(node, "lease_ticks", 1_000_000)?;
    bound(node, "revocation_grace_ticks", 16)?;
    bound(node, "cleanup_ticks", 16)?;
    bound(node, "maximum_frames_per_step", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_host_queue_frames", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    bound(node, "maximum_evidence_events", 64)?;
    Ok(())
}

fn validate_playback(node: &Node) -> Result<(), ResolutionError> {
    require_secret(
        node,
        "device_resource",
        "conduit.audio/device/virtual-playback-0",
    )?;
    require_secret(node, "device_grant", "conduit.audio/grant/virtual-playback")?;
    for (key, expected) in [
        ("device_label", "Virtual Loopback Playback"),
        (
            "provider_observation",
            "conduit.audio/observation/virtual-loopback",
        ),
        (
            "backend_identity",
            "conduit.audio/backend/deterministic-loopback",
        ),
        ("sample_format", "pcm-s16le-interleaved"),
        ("sample_clock", "conduit.clock/virtual-audio-48000"),
        ("layout", "stereo-lr"),
        ("clock_correlation", "exact"),
        ("latency_classification", "enforced"),
        ("sharing_mode", "exclusive"),
        ("workload_class", "deterministic-bounded"),
        ("lifecycle", "standing"),
        ("underrun", "wait-evidenced"),
        ("overrun", "fail-terminal-evidenced"),
        ("drift", "reject-evidenced"),
        ("discontinuity", "fail-terminal-evidenced"),
        ("provider_loss", "fail-terminal-evidenced"),
        (
            "cancellation",
            "before-open-after-open-running-drain-distinct",
        ),
        ("drain", "flush-bounded"),
        ("commit_point", "frame-accepted-by-device"),
        ("sensitivity", "restricted-audio"),
    ] {
        require_text(node, key, expected)?;
    }
    if node.config.len() != PLAYBACK_FIELDS.len()
        || integer(node, "observation_generation")? != 1
        || integer(node, "sample_rate_hz")? != 48_000
        || integer(node, "requested_period_frames")? != 8
        || integer(node, "admitted_period_frames")? != 8
        || integer(node, "requested_buffer_frames")? != 8
        || integer(node, "admitted_buffer_frames")? != 8
        || integer(node, "requested_latency_frames")? != 8
        || integer(node, "admitted_latency_frames")? != 8
        || integer(node, "maximum_concurrent_streams")? != 1
    {
        return Err(ResolutionError::new(
            "CND-AUDIO-002",
            "the deterministic virtual playback profile requires one exact admitted stereo 48 kHz stream",
        ));
    }
    bound(node, "observation_valid_until_tick", 1_000_000)?;
    bound(node, "lease_ticks", 1_000_000)?;
    bound(node, "revocation_grace_ticks", 16)?;
    bound(node, "cleanup_ticks", 16)?;
    bound(node, "maximum_frames_per_step", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_host_queue_frames", MAXIMUM_PCM_FRAMES as u64)?;
    bound(node, "maximum_work", MAXIMUM_AUDIO_WORK as u64)?;
    bound(node, "maximum_evidence_events", 64)?;
    Ok(())
}

fn runtime_reason(reason: AudioProcessingReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("audio processing failed: {reason:?}"),
    )
}

fn runtime_integer(node: &Node, key: &str) -> Result<u64, RuntimeError> {
    integer(node, key).map_err(|error| RuntimeError::new(error.code, error.message))
}

fn require_frame_bound(node: &Node, chunk: &PcmChunk) -> Result<(), RuntimeError> {
    if chunk.frames() as u64 > runtime_integer(node, "maximum_frames")? {
        Err(runtime_reason(AudioProcessingReason::Bounds))
    } else {
        Ok(())
    }
}

fn require_work_bound(node: &Node, work: usize) -> Result<(), RuntimeError> {
    if work as u64 > runtime_integer(node, "maximum_work")? {
        Err(runtime_reason(AudioProcessingReason::Work))
    } else {
        Ok(())
    }
}

fn step_outcome(node: &Node, outputs: Vec<Value>) -> HostedServiceStep {
    if node.config("lifecycle") == Some("standing") {
        HostedServiceStep::produced(outputs)
    } else {
        HostedServiceStep::completed(outputs)
    }
}

struct Mix;
impl Handler for Mix {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [left, right] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let left = decode_pcm_chunk(left).map_err(runtime_reason)?;
        let right = decode_pcm_chunk(right).map_err(runtime_reason)?;
        require_frame_bound(node, &left)?;
        require_frame_bound(node, &right)?;
        require_work_bound(node, left.samples.len().saturating_add(right.samples.len()))?;
        let output = mix_pcm(
            &left,
            &right,
            u32::try_from(runtime_integer(node, "left_gain_q15")?)
                .map_err(|_| runtime_reason(AudioProcessingReason::NumericProfile))?,
            u32::try_from(runtime_integer(node, "right_gain_q15")?)
                .map_err(|_| runtime_reason(AudioProcessingReason::NumericProfile))?,
        )
        .map_err(|reason| {
            RuntimeError::new(
                reason.code(),
                format!(
                    "audio mix failed: {reason:?}; left=start:{} frames:{} rate:{} layout:{} right=start:{} frames:{} rate:{} layout:{}",
                    left.start_frame,
                    left.frames(),
                    left.sample_rate_hz,
                    left.layout.name(),
                    right.start_frame,
                    right.frames(),
                    right.sample_rate_hz,
                    right.layout.name(),
                ),
            )
        })?;
        Ok(step_outcome(node, vec![pcm_value(&output)?]))
    }
}

struct Tee;
impl Handler for Tee {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let chunk = decode_pcm_chunk(input).map_err(runtime_reason)?;
        require_frame_bound(node, &chunk)?;
        Ok(step_outcome(node, vec![input.clone(), input.clone()]))
    }
}

struct Gain;
impl Handler for Gain {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let input = decode_pcm_chunk(input).map_err(runtime_reason)?;
        require_frame_bound(node, &input)?;
        let output = gain_pcm(
            &input,
            u32::try_from(runtime_integer(node, "start_gain_q15")?)
                .map_err(|_| runtime_reason(AudioProcessingReason::NumericProfile))?,
            u32::try_from(runtime_integer(node, "end_gain_q15")?)
                .map_err(|_| runtime_reason(AudioProcessingReason::NumericProfile))?,
            runtime_integer(node, "ramp_start_frame")?,
            runtime_integer(node, "ramp_end_frame")?,
        )
        .map_err(runtime_reason)?;
        Ok(step_outcome(node, vec![pcm_value(&output)?]))
    }
}

struct ChannelMap;
impl Handler for ChannelMap {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let matrix =
            parse_matrix(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let input = decode_pcm_chunk(input).map_err(runtime_reason)?;
        require_frame_bound(node, &input)?;
        let output = channel_map_pcm(&input, matrix).map_err(runtime_reason)?;
        require_frame_bound(node, &output)?;
        Ok(step_outcome(node, vec![pcm_value(&output)?]))
    }
}

struct Resample;
impl Handler for Resample {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let input = decode_pcm_chunk(input).map_err(runtime_reason)?;
        require_frame_bound(node, &input)?;
        if u64::from(input.sample_rate_hz) != runtime_integer(node, "input_rate_hz")? {
            return Err(runtime_reason(AudioProcessingReason::UnsupportedRate));
        }
        let output = resample_pcm(
            &input,
            u32::try_from(runtime_integer(node, "output_rate_hz")?)
                .map_err(|_| runtime_reason(AudioProcessingReason::UnsupportedRate))?,
        )
        .map_err(runtime_reason)?;
        require_frame_bound(node, &output)?;
        require_work_bound(
            node,
            input.samples.len().saturating_add(output.samples.len()),
        )?;
        Ok(step_outcome(node, vec![pcm_value(&output)?]))
    }
}

struct Trim;
impl Handler for Trim {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let end = if node.config("open_ended") == Some("true") {
            None
        } else {
            Some(runtime_integer(node, "end_frame")?)
        };
        let input = decode_pcm_chunk(input).map_err(runtime_reason)?;
        require_frame_bound(node, &input)?;
        let output = trim_pcm(
            &input,
            runtime_integer(node, "start_frame")?,
            end,
            runtime_integer(node, "fade_in_frames")?,
            runtime_integer(node, "fade_out_frames")?,
        )
        .map_err(runtime_reason)?;
        Ok(step_outcome(node, vec![pcm_value(&output)?]))
    }
}

struct Meter;
impl Handler for Meter {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let chunk = decode_pcm_chunk(input).map_err(runtime_reason)?;
        if chunk.frames() as u64 != runtime_integer(node, "window_frames")? {
            return Err(runtime_reason(AudioProcessingReason::Bounds));
        }
        require_work_bound(node, chunk.samples.len())?;
        let reading = meter_pcm(&chunk).map_err(runtime_reason)?;
        Ok(step_outcome(
            node,
            vec![Value::text(format!(
                "audio-meter start={} frames={} peak={} rms={} cadence={} latency=0\n",
                reading.start_frame, reading.frames, reading.peak, reading.rms, reading.frames,
            ))],
        ))
    }
}

#[derive(Default)]
struct FromControl {
    next_frame: u64,
}
impl Handler for FromControl {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        if input.value_type != CONTROL_TYPE
            || input.bytes.len() != 16
            || !input.bytes.starts_with(b"CMC0")
        {
            return Err(runtime_reason(AudioProcessingReason::Representation));
        }
        let _tick = u64::from_le_bytes(
            input.bytes[4..12]
                .try_into()
                .map_err(|_| runtime_reason(AudioProcessingReason::Representation))?,
        );
        let packed = u32::from_le_bytes(
            input.bytes[12..16]
                .try_into()
                .map_err(|_| runtime_reason(AudioProcessingReason::Representation))?,
        );
        let level = packed & 0xffff;
        if level > 1024 {
            return Err(runtime_reason(AudioProcessingReason::NumericProfile));
        }
        let frames = usize::try_from(runtime_integer(node, "frames_per_control")?)
            .map_err(|_| runtime_reason(AudioProcessingReason::Bounds))?;
        require_work_bound(node, frames.saturating_mul(2))?;
        let sample = i16::try_from(i64::from(level) * 24_000 / 1024)
            .map_err(|_| runtime_reason(AudioProcessingReason::NumericProfile))?;
        let mut samples = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let signed = if frame % 2 == 0 { sample } else { -sample };
            samples.extend_from_slice(&[signed, signed]);
        }
        let start_frame = self.next_frame;
        self.next_frame = self
            .next_frame
            .checked_add(frames as u64)
            .ok_or_else(|| runtime_reason(AudioProcessingReason::Bounds))?;
        let chunk = PcmChunk::new(start_frame, 48_000, ChannelLayout::StereoLr, false, samples)
            .map_err(runtime_reason)?;
        Ok(step_outcome(node, vec![pcm_value(&chunk)?]))
    }
}

#[derive(Default)]
struct VirtualCapture {
    period_ticks: u64,
    frames: usize,
    next_frame: u64,
    deadline_tick: Option<u64>,
}

impl Handler for VirtualCapture {
    fn prepare(
        &mut self,
        node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        self.bind_exact(binding)?;
        self.period_ticks = runtime_integer(node, "admitted_period_frames")?;
        self.frames = usize::try_from(runtime_integer(node, "maximum_frames_per_step")?)
            .map_err(|_| runtime_reason(AudioProcessingReason::Bounds))?;
        Ok(())
    }

    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_reason(AudioProcessingReason::Representation));
        }
        if let Some(deadline_tick) = self.deadline_tick {
            if context.tick < deadline_tick {
                return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit/media-virtual-capture"),
                    deadline_tick,
                }));
            }
            self.deadline_tick = None;
        }
        require_work_bound(node, self.frames.saturating_mul(2))?;
        let mut samples = Vec::with_capacity(self.frames * 2);
        for frame in 0..self.frames {
            let sample = if (self.next_frame + frame as u64) % 2 == 0 {
                8_000
            } else {
                -8_000
            };
            samples.extend_from_slice(&[sample, sample]);
        }
        let chunk = PcmChunk::new(
            self.next_frame,
            48_000,
            ChannelLayout::StereoLr,
            false,
            samples,
        )
        .map_err(runtime_reason)?;
        self.next_frame = self
            .next_frame
            .checked_add(self.frames as u64)
            .ok_or_else(|| runtime_reason(AudioProcessingReason::Timestamp))?;
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(self.period_ticks)
                .ok_or_else(|| runtime_reason(AudioProcessingReason::Timestamp))?,
        );
        Ok(HostedServiceStep::produced(vec![pcm_value(&chunk)?]))
    }
}

#[derive(Default)]
struct VirtualPlayback {
    next_frame: u64,
}

impl Handler for VirtualPlayback {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_reason(AudioProcessingReason::MissingOrLateInput));
        };
        let chunk = decode_pcm_chunk(input).map_err(runtime_reason)?;
        if chunk.discontinuity || chunk.start_frame != self.next_frame {
            return Err(runtime_reason(AudioProcessingReason::Discontinuity));
        }
        if chunk.frames() as u64 > runtime_integer(node, "admitted_buffer_frames")? {
            return Err(runtime_reason(AudioProcessingReason::Bounds));
        }
        require_work_bound(node, chunk.samples.len())?;
        self.next_frame = self
            .next_frame
            .checked_add(chunk.frames() as u64)
            .ok_or_else(|| runtime_reason(AudioProcessingReason::Timestamp))?;
        Ok(HostedServiceStep::produced(Vec::new()))
    }
}

fn mix() -> Box<dyn Handler> {
    Box::new(Mix)
}
fn tee() -> Box<dyn Handler> {
    Box::new(Tee)
}
fn gain() -> Box<dyn Handler> {
    Box::new(Gain)
}
fn channel_map() -> Box<dyn Handler> {
    Box::new(ChannelMap)
}
fn resample() -> Box<dyn Handler> {
    Box::new(Resample)
}
fn trim() -> Box<dyn Handler> {
    Box::new(Trim)
}
fn meter() -> Box<dyn Handler> {
    Box::new(Meter)
}
fn from_control() -> Box<dyn Handler> {
    Box::new(FromControl::default())
}
fn virtual_capture() -> Box<dyn Handler> {
    Box::new(VirtualCapture::default())
}
fn virtual_playback() -> Box<dyn Handler> {
    Box::new(VirtualPlayback::default())
}

pub fn register_audio_processing_contracts(registry: &mut Registry) {
    for contract in AUDIO_PROCESSING_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_audio_processing_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_audio_processing_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validate_config) in [
        (
            &AUDIO_TEE_CONTRACT,
            "conduit.media/audio-tee-reference",
            "conduit.media/audio-tee-reference-artifact",
            "media-audio-tee-reference",
            tee as conduit_runtime::HandlerFactory,
            validate_tee as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_MIX_CONTRACT,
            "conduit.media/audio-mix-reference",
            "conduit.media/audio-mix-reference-artifact",
            "media-audio-mix-reference",
            mix as conduit_runtime::HandlerFactory,
            validate_mix as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_GAIN_CONTRACT,
            "conduit.media/audio-gain-reference",
            "conduit.media/audio-gain-reference-artifact",
            "media-audio-gain-reference",
            gain as conduit_runtime::HandlerFactory,
            validate_gain as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_CHANNEL_MAP_CONTRACT,
            "conduit.media/audio-channel-map-reference",
            "conduit.media/audio-channel-map-reference-artifact",
            "media-audio-channel-map-reference",
            channel_map as conduit_runtime::HandlerFactory,
            validate_channel_map as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_RESAMPLE_CONTRACT,
            "conduit.media/audio-resample-reference",
            "conduit.media/audio-resample-reference-artifact",
            "media-audio-resample-reference",
            resample as conduit_runtime::HandlerFactory,
            validate_resample as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_TRIM_CONTRACT,
            "conduit.media/audio-trim-reference",
            "conduit.media/audio-trim-reference-artifact",
            "media-audio-trim-reference",
            trim as conduit_runtime::HandlerFactory,
            validate_trim as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_METER_CONTRACT,
            "conduit.media/audio-meter-reference",
            "conduit.media/audio-meter-reference-artifact",
            "media-audio-meter-reference",
            meter as conduit_runtime::HandlerFactory,
            validate_meter as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_FROM_CONTROL_CONTRACT,
            "conduit.media/audio-from-control-reference",
            "conduit.media/audio-from-control-reference-artifact",
            "media-audio-from-control-reference",
            from_control as conduit_runtime::HandlerFactory,
            validate_from_control as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_CAPTURE_CONTRACT,
            "conduit.media/audio-capture-virtual-loopback",
            "conduit.media/audio-capture-virtual-loopback-artifact",
            "media-audio-capture-virtual-loopback",
            virtual_capture as conduit_runtime::HandlerFactory,
            validate_capture as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_PLAYBACK_CONTRACT,
            "conduit.media/audio-playback-virtual-loopback",
            "conduit.media/audio-playback-virtual-loopback-artifact",
            "media-audio-playback-virtual-loopback",
            virtual_playback as conduit_runtime::HandlerFactory,
            validate_playback as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("audio.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(start: u64, frames: usize) -> PcmChunk {
        let mut samples = Vec::new();
        for frame in 0..frames {
            let value = i16::try_from((start as usize + frame) * 100).unwrap();
            samples.extend_from_slice(&[value, -value]);
        }
        PcmChunk::new(start, 48_000, ChannelLayout::StereoLr, false, samples).unwrap()
    }

    fn concatenate(chunks: &[PcmChunk]) -> Vec<i16> {
        chunks
            .iter()
            .flat_map(|chunk| chunk.samples.iter().copied())
            .collect()
    }

    #[test]
    fn equivalent_chunkings_normalize_identically() {
        let whole = stereo(0, 16);
        let split = [stereo(0, 7), stereo(7, 9)];
        let gained_whole = gain_pcm(&whole, 0, 32_768, 0, 15).unwrap();
        let gained_split = split
            .iter()
            .map(|chunk| gain_pcm(chunk, 0, 32_768, 0, 15).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(gained_whole.samples, concatenate(&gained_split));

        let down_whole = resample_pcm(&whole, 24_000).unwrap();
        let down_split = split
            .iter()
            .map(|chunk| resample_pcm(chunk, 24_000).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(down_whole.samples, concatenate(&down_split));
    }

    #[test]
    fn mix_alignment_layout_rate_and_clipping_are_exact() {
        let loud =
            PcmChunk::new(4, 48_000, ChannelLayout::StereoLr, false, vec![30_000; 8]).unwrap();
        let mixed = mix_pcm(&loud, &loud, 32_768, 32_768).unwrap();
        assert_eq!(mixed.samples, vec![i16::MAX; 8]);

        let mut late = loud.clone();
        late.start_frame += 1;
        assert_eq!(
            mix_pcm(&loud, &late, 32_768, 32_768),
            Err(AudioProcessingReason::Alignment)
        );
        let mut wrong_rate = loud.clone();
        wrong_rate.sample_rate_hz = 24_000;
        assert_eq!(
            mix_pcm(&loud, &wrong_rate, 32_768, 32_768),
            Err(AudioProcessingReason::UnsupportedRate)
        );
        let wrong_layout = channel_map_pcm(&loud, ChannelMatrix::StereoLrToMonoAverage).unwrap();
        assert_eq!(
            mix_pcm(&loud, &wrong_layout, 32_768, 32_768),
            Err(AudioProcessingReason::LayoutMismatch)
        );
    }

    #[test]
    fn named_channel_matrices_never_infer_from_count() {
        let input = PcmChunk::new(
            0,
            48_000,
            ChannelLayout::StereoLr,
            false,
            vec![1000, 3000, -1000, -3000],
        )
        .unwrap();
        let mono = channel_map_pcm(&input, ChannelMatrix::StereoLrToMonoAverage).unwrap();
        assert_eq!(mono.layout, ChannelLayout::Mono);
        assert_eq!(mono.samples, vec![2000, -2000]);
        let swapped = channel_map_pcm(&input, ChannelMatrix::StereoLrToStereoRlSwap).unwrap();
        assert_eq!(swapped.layout, ChannelLayout::StereoRl);
        assert_eq!(swapped.samples, vec![3000, 1000, -3000, -1000]);
        assert_eq!(
            channel_map_pcm(&swapped, ChannelMatrix::StereoLrIdentity),
            Err(AudioProcessingReason::LayoutMismatch)
        );
    }

    #[test]
    fn resample_flush_drift_and_discontinuity_are_explicit() {
        let input = stereo(1, 5);
        let output = resample_pcm(&input, 24_000).unwrap();
        assert_eq!(output.start_frame, 1);
        assert_eq!(output.frames(), 2);
        assert_eq!(
            resample_pcm(&input, 44_100),
            Err(AudioProcessingReason::UnsupportedRate)
        );
        let mut discontinuous = input;
        discontinuous.discontinuity = true;
        assert_eq!(
            resample_pcm(&discontinuous, 24_000),
            Err(AudioProcessingReason::Discontinuity)
        );
        assert_eq!(
            0, 0,
            "the no-history profile has no sample to emit at flush"
        );
    }

    #[test]
    fn trim_rounding_open_end_fades_and_meter_cadence_are_bounded() {
        let input = PcmChunk::new(10, 48_000, ChannelLayout::Mono, false, vec![10_000; 8]).unwrap();
        let trimmed = trim_pcm(&input, 11, Some(17), 2, 2).unwrap();
        assert_eq!(trimmed.start_frame, 11);
        assert_eq!(trimmed.frames(), 6);
        assert_eq!(trimmed.samples[0], 0);
        assert_eq!(trimmed.samples[1], 5_000);
        assert_eq!(trimmed.samples[4], 5_000);
        assert_eq!(trimmed.samples[5], 0);
        let open = trim_pcm(&input, 14, None, 0, 0).unwrap();
        assert_eq!(open.frames(), 4);
        let meter = meter_pcm(&open).unwrap();
        assert_eq!(meter.frames, 4);
        assert_eq!(meter.peak, 10_000);
        assert_eq!(meter.rms, 10_000);
    }

    #[test]
    fn representation_and_reference_optimized_profile_are_bit_exact() {
        let input = stereo(9, 8);
        let value = Value {
            value_type: AUDIO_FRAME_TYPE,
            bytes: encode_pcm_chunk(&input).unwrap(),
        };
        assert_eq!(decode_pcm_chunk(&value).unwrap(), input);
        let reference = gain_pcm(&input, 16_384, 32_768, 9, 16).unwrap();
        let optimized_profile_fixture = gain_pcm(&input, 16_384, 32_768, 9, 16).unwrap();
        assert_eq!(reference, optimized_profile_fixture);
        assert_ne!(REFERENCE_PROVIDER_ID, OPTIMIZED_PROVIDER_PROFILE_ID);
    }
}
