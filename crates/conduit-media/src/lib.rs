//! Host-neutral bounded media value contracts.
//!
//! This crate defines values and exact compatibility only. It does not expose
//! codecs, devices, host discovery, implicit conversion, or another event
//! model.

mod audio;
mod codec;
mod signal;

pub use audio::*;
pub use codec::*;
pub use signal::*;

use sha2::{Digest, Sha256};

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::Node;
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

pub const MAXIMUM_PLANES: usize = 4;
pub const MAXIMUM_CHANNELS: u16 = 64;
pub const MAXIMUM_METADATA_ENTRIES: u16 = 64;
pub const MAXIMUM_METADATA_BYTES: usize = 64 * 1024;
pub const MAXIMUM_MEDIA_BYTES: usize = 16 * 1024 * 1024;

pub const AUDIO_FRAME_DESCRIPTOR: &str = "conduit.media/audio-frame|0|s16le,s24le,f32le|rational-time|finite-planes-strides-frames-bytes";
pub const VIDEO_FRAME_DESCRIPTOR: &str = "conduit.media/video-frame|0|rgb24,rgba32,gray8,yuv420p|rational-time|finite-dimensions-planes-strides-bytes";

pub const AUDIO_FRAME_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/audio-frame"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x10, 0x7e, 0xf4, 0x1b, 0xaa, 0xa8, 0x1b, 0x35, 0x67, 0x51, 0x3d, 0xd1, 0xd9, 0xf4, 0x04,
        0xae, 0xe4, 0x83, 0x13, 0x91, 0x46, 0x43, 0xa0, 0x31, 0xfb, 0x97, 0x44, 0xcf, 0x49, 0x84,
        0x20, 0xea,
    ]),
};
pub const VIDEO_FRAME_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/video-frame"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x7d, 0x91, 0xb0, 0x94, 0x81, 0xaa, 0xef, 0x99, 0xe1, 0xd1, 0xfd, 0x72, 0x70, 0x0a, 0x64,
        0x8d, 0x3a, 0xce, 0x31, 0x9c, 0xc9, 0xa7, 0xea, 0x8b, 0x71, 0x22, 0xce, 0x17, 0x6d, 0xea,
        0xb2, 0x0f,
    ]),
};
pub(crate) const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const AUDIO_OUTPUT: [PortContract<'static>; 1] =
    [port("frame", Direction::Output, AUDIO_FRAME_TYPE)];
const AUDIO_INPUT: [PortContract<'static>; 1] = [PortContract {
    id: Id("frame"),
    direction: Direction::Input,
    value_type: AUDIO_FRAME_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Either,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
}];
const VIDEO_OUTPUT: [PortContract<'static>; 1] =
    [port("frame", Direction::Output, VIDEO_FRAME_TYPE)];
const VIDEO_INPUT: [PortContract<'static>; 1] = [port("frame", Direction::Input, VIDEO_FRAME_TYPE)];
const TEXT_OUTPUT: [PortContract<'static>; 1] = [PortContract {
    id: Id("summary"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
}];
const FIXTURE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[ConfigFieldContract {
        key: Id("fixture"),
        value_type: TEXT_TYPE,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }],
};

pub const AUDIO_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio-frame/literal"),
    config: FIXTURE_CONFIG,
    inputs: &[],
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio-frame/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &AUDIO_INPUT,
    outputs: &TEXT_OUTPUT,
};
pub const VIDEO_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/video-frame/literal"),
    config: FIXTURE_CONFIG,
    inputs: &[],
    outputs: &VIDEO_OUTPUT,
};
pub const VIDEO_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/video-frame/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &VIDEO_INPUT,
    outputs: &TEXT_OUTPUT,
};

pub(crate) const AUDIO_VALUE: &[u8] = b"CMA0T\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xc0synthetic-pcm";
pub(crate) const AUDIO_SILENCE_VALUE: &[u8] = b"CMA0S\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xc0synthetic-pcm";
const VIDEO_VALUE: &[u8] = b"CMV0R\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01synthetic-rgb";
const VIDEO_GRAY_VALUE: &[u8] = b"CMV0G\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01synthetic-gray";

struct LiteralHandler {
    value_type: TypeContractRef<'static>,
    primary: (&'static str, &'static [u8]),
    alternate: (&'static str, &'static [u8]),
}

impl Handler for LiteralHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-MEDIA-001",
                "media literal received hidden input",
            ));
        }
        let bytes = match node.config("fixture") {
            Some(name) if name == self.primary.0 => self.primary.1,
            Some(name) if name == self.alternate.0 => self.alternate.1,
            _ => {
                return Err(RuntimeError::new(
                    "CND-MEDIA-004",
                    "media fixture selection is invalid",
                ));
            }
        };
        Ok(vec![Value {
            value_type: self.value_type,
            bytes: bytes.to_vec(),
        }])
    }
}

struct InspectHandler {
    expected_type: TypeContractRef<'static>,
    magic: &'static [u8; 4],
    primary: (u8, &'static [u8]),
    alternate: (u8, &'static [u8]),
}

impl Handler for InspectHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MEDIA-002",
                "media inspector requires one frame",
            ));
        };
        if self.expected_type == AUDIO_FRAME_TYPE && input.bytes.starts_with(b"CAP0") {
            let chunk = decode_pcm_chunk(input).map_err(|reason| {
                RuntimeError::new(
                    reason.code(),
                    "processed PCM frame representation is invalid",
                )
            })?;
            return Ok(vec![Value::text(format!(
                "audio:s16le:{}:{}:{}",
                chunk.sample_rate_hz,
                chunk.layout.name(),
                chunk.frames()
            ))]);
        }
        if input.value_type != self.expected_type
            || input.bytes.len() > 64
            || !input.bytes.starts_with(self.magic)
        {
            return Err(RuntimeError::new(
                "CND-MEDIA-003",
                "media frame representation is invalid",
            ));
        }
        let summary = match input.bytes.get(4) {
            Some(marker) if *marker == self.primary.0 => self.primary.1,
            Some(marker) if *marker == self.alternate.0 => self.alternate.1,
            _ => {
                return Err(RuntimeError::new(
                    "CND-MEDIA-003",
                    "media frame fixture marker is invalid",
                ));
            }
        };
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: summary.to_vec(),
        }])
    }
}

fn audio_literal() -> Box<dyn Handler> {
    Box::new(LiteralHandler {
        value_type: AUDIO_FRAME_TYPE,
        primary: ("tone-s16le-stereo-48000", AUDIO_VALUE),
        alternate: ("silence-s16le-stereo-48000", AUDIO_SILENCE_VALUE),
    })
}
fn video_literal() -> Box<dyn Handler> {
    Box::new(LiteralHandler {
        value_type: VIDEO_FRAME_TYPE,
        primary: ("rgb24-2x2", VIDEO_VALUE),
        alternate: ("gray8-2x2", VIDEO_GRAY_VALUE),
    })
}
fn audio_inspect() -> Box<dyn Handler> {
    Box::new(InspectHandler {
        expected_type: AUDIO_FRAME_TYPE,
        magic: b"CMA0",
        primary: (b'T', b"audio:s16le:48000:stereo:192"),
        alternate: (b'S', b"audio:s16le:48000:stereo:192"),
    })
}
fn video_inspect() -> Box<dyn Handler> {
    Box::new(InspectHandler {
        expected_type: VIDEO_FRAME_TYPE,
        magic: b"CMV0",
        primary: (b'R', b"video:rgb24:2x2"),
        alternate: (b'G', b"video:gray8:2x2"),
    })
}

fn validate_fixture_config(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() == 1
        && matches!(
            node.config("fixture"),
            Some(
                "tone-s16le-stereo-48000"
                    | "silence-s16le-stereo-48000"
                    | "rgb24-2x2"
                    | "gray8-2x2"
            )
        )
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MEDIA-004",
            "synthetic media fixture selection is unsupported",
        ))
    }
}

pub fn register_media_contracts(registry: &mut Registry) {
    for contract in [
        &AUDIO_LITERAL_CONTRACT,
        &AUDIO_INSPECT_CONTRACT,
        &VIDEO_LITERAL_CONTRACT,
        &VIDEO_INSPECT_CONTRACT,
    ] {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_media_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_media_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validate_config) in [
        (
            &AUDIO_LITERAL_CONTRACT,
            "conduit.media/audio-literal-deterministic",
            "conduit.media/audio-literal-artifact",
            "media-audio-literal",
            audio_literal as conduit_runtime::HandlerFactory,
            validate_fixture_config as conduit_runtime::ConfigValidator,
        ),
        (
            &AUDIO_INSPECT_CONTRACT,
            "conduit.media/audio-inspect-deterministic",
            "conduit.media/audio-inspect-artifact",
            "media-audio-inspect",
            audio_inspect as conduit_runtime::HandlerFactory,
            (|node: &Node| {
                node.config.is_empty().then_some(()).ok_or_else(|| {
                    ResolutionError::new("CND-MEDIA-004", "media inspector has no configuration")
                })
            }) as conduit_runtime::ConfigValidator,
        ),
        (
            &VIDEO_LITERAL_CONTRACT,
            "conduit.media/video-literal-deterministic",
            "conduit.media/video-literal-artifact",
            "media-video-literal",
            video_literal as conduit_runtime::HandlerFactory,
            validate_fixture_config as conduit_runtime::ConfigValidator,
        ),
        (
            &VIDEO_INSPECT_CONTRACT,
            "conduit.media/video-inspect-deterministic",
            "conduit.media/video-inspect-artifact",
            "media-video-inspect",
            video_inspect as conduit_runtime::HandlerFactory,
            (|node: &Node| {
                node.config.is_empty().then_some(()).ok_or_else(|| {
                    ResolutionError::new("CND-MEDIA-004", "media inspector has no configuration")
                })
            }) as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config,
        })?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaReason {
    InvalidTimeBase,
    MissingTimestamp,
    InvalidDuration,
    InvalidDimensions,
    InvalidPlaneLayout,
    UnsupportedFormat,
    ChannelLayoutMismatch,
    DescriptorMismatch,
    MetadataOverflow,
    ByteOverflow,
    DuplicateTimestamp,
    IncompatibleTimeBase,
    PacketExtradataMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

impl RationalTimeBase {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.numerator == 0 || self.denominator == 0 {
            return Err(MediaReason::InvalidTimeBase);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaTime {
    pub time_base: RationalTimeBase,
    pub timestamp: Option<i64>,
    pub duration: u64,
    pub discontinuity: bool,
    pub conversion_uncertainty_ticks: u64,
}

impl MediaTime {
    pub fn validate(self) -> Result<(), MediaReason> {
        self.time_base.validate()?;
        if self.timestamp.is_none() {
            return Err(MediaReason::MissingTimestamp);
        }
        if self.duration == 0 {
            return Err(MediaReason::InvalidDuration);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockCorrelation {
    pub media_timestamp: i64,
    pub host_tick: u64,
    pub uncertainty_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamDescriptor {
    pub identity: [u8; 32],
    pub time_base: RationalTimeBase,
    pub maximum_frames_per_value: u32,
    pub maximum_value_bytes: usize,
    pub maximum_metadata_entries: u16,
    pub maximum_buffered_values: u16,
}

impl StreamDescriptor {
    pub fn validate(self) -> Result<(), MediaReason> {
        self.time_base.validate()?;
        if self.identity == [0; 32]
            || self.maximum_frames_per_value == 0
            || self.maximum_value_bytes == 0
            || self.maximum_value_bytes > MAXIMUM_MEDIA_BYTES
            || self.maximum_metadata_entries > MAXIMUM_METADATA_ENTRIES
            || self.maximum_buffered_values == 0
        {
            return Err(MediaReason::ByteOverflow);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataEntry<'a> {
    pub key: &'a str,
    pub value: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaMetadata<'a> {
    pub entries: &'a [MetadataEntry<'a>],
    pub provenance_identity: [u8; 32],
    pub sensitivity: Sensitivity,
}

impl MediaMetadata<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.entries.len() > usize::from(MAXIMUM_METADATA_ENTRIES)
            || self.provenance_identity == [0; 32]
        {
            return Err(MediaReason::MetadataOverflow);
        }
        let mut bytes = 0_usize;
        for entry in self.entries {
            if entry.key.is_empty()
                || entry.key.len() > 255
                || entry.value.len() > MAXIMUM_METADATA_BYTES
            {
                return Err(MediaReason::MetadataOverflow);
            }
            bytes = bytes
                .checked_add(entry.key.len())
                .and_then(|total| total.checked_add(entry.value.len()))
                .ok_or(MediaReason::MetadataOverflow)?;
            if bytes > MAXIMUM_METADATA_BYTES {
                return Err(MediaReason::MetadataOverflow);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaValueHeader<'a> {
    pub stream: StreamDescriptor,
    pub time: MediaTime,
    pub clock_correlation: Option<ClockCorrelation>,
    pub metadata: MediaMetadata<'a>,
}

impl MediaValueHeader<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        self.stream.validate()?;
        self.time.validate()?;
        self.metadata.validate()?;
        if self.stream.time_base != self.time.time_base {
            return Err(MediaReason::IncompatibleTimeBase);
        }
        if let Some(correlation) = self.clock_correlation
            && correlation.media_timestamp != self.time.timestamp.expect("validated timestamp")
        {
            return Err(MediaReason::IncompatibleTimeBase);
        }
        Ok(())
    }
}

pub fn validate_timestamp_sequence(values: &[MediaTime]) -> Result<(), MediaReason> {
    let mut previous = None;
    for value in values {
        value.validate()?;
        let timestamp = value.timestamp.expect("validated timestamp");
        if previous == Some(timestamp) {
            return Err(MediaReason::DuplicateTimestamp);
        }
        if previous.is_some_and(|previous| previous > timestamp) {
            return Err(MediaReason::IncompatibleTimeBase);
        }
        previous = Some(timestamp);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plane {
    pub offset: usize,
    pub stride: usize,
    pub rows: usize,
}

impl Plane {
    fn end(self) -> Option<usize> {
        self.stride
            .checked_mul(self.rows)
            .and_then(|bytes| self.offset.checked_add(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioDescriptor<'a> {
    pub sample_format: &'a str,
    pub sample_rate_hz: u32,
    pub channel_layout: &'a str,
    pub channels: u16,
    pub frames: u32,
    pub planes: &'a [Plane],
    pub maximum_bytes: usize,
}

impl AudioDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if !matches!(self.sample_format, "s16le" | "s24le" | "f32le") || self.sample_rate_hz == 0 {
            return Err(MediaReason::UnsupportedFormat);
        }
        if self.channels == 0 || self.channels > MAXIMUM_CHANNELS || self.frames == 0 {
            return Err(MediaReason::ChannelLayoutMismatch);
        }
        validate_planes(self.planes, self.maximum_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoDescriptor<'a> {
    pub width: u32,
    pub height: u32,
    pub pixel_format: &'a str,
    pub color_space: &'a str,
    pub color_range: &'a str,
    pub transfer: &'a str,
    pub orientation_degrees: u16,
    pub alpha: bool,
    pub planes: &'a [Plane],
    pub maximum_bytes: usize,
}

impl VideoDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.width == 0 || self.height == 0 {
            return Err(MediaReason::InvalidDimensions);
        }
        if !matches!(self.pixel_format, "rgb24" | "rgba32" | "gray8" | "yuv420p") {
            return Err(MediaReason::UnsupportedFormat);
        }
        if !matches!(self.orientation_degrees, 0 | 90 | 180 | 270) {
            return Err(MediaReason::InvalidDimensions);
        }
        validate_planes(self.planes, self.maximum_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketDescriptor<'a> {
    pub codec: &'a str,
    pub profile: &'a str,
    pub extradata_identity: [u8; 32],
    pub key: bool,
    pub discontinuity: bool,
    pub time: MediaTime,
    pub maximum_bytes: usize,
}

impl PacketDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        self.time.validate()?;
        if self.codec.is_empty() || self.profile.is_empty() {
            return Err(MediaReason::UnsupportedFormat);
        }
        if self.maximum_bytes == 0 || self.maximum_bytes > MAXIMUM_MEDIA_BYTES {
            return Err(MediaReason::ByteOverflow);
        }
        Ok(())
    }
}

fn validate_planes(planes: &[Plane], maximum_bytes: usize) -> Result<(), MediaReason> {
    if planes.is_empty()
        || planes.len() > MAXIMUM_PLANES
        || maximum_bytes == 0
        || maximum_bytes > MAXIMUM_MEDIA_BYTES
        || planes
            .iter()
            .any(|plane| plane.stride == 0 || plane.rows == 0 || plane.end() > Some(maximum_bytes))
    {
        return Err(MediaReason::InvalidPlaneLayout);
    }
    Ok(())
}

#[must_use]
pub fn descriptor_hash(canonical_descriptor: &str) -> [u8; 32] {
    Sha256::digest(canonical_descriptor.as_bytes()).into()
}

pub fn exact_audio_compatibility(
    producer: AudioDescriptor<'_>,
    consumer: AudioDescriptor<'_>,
) -> Result<(), MediaReason> {
    producer.validate()?;
    consumer.validate()?;
    if producer == consumer {
        Ok(())
    } else if producer.channel_layout != consumer.channel_layout
        || producer.channels != consumer.channels
    {
        Err(MediaReason::ChannelLayoutMismatch)
    } else {
        Err(MediaReason::DescriptorMismatch)
    }
}

pub fn exact_video_compatibility(
    producer: VideoDescriptor<'_>,
    consumer: VideoDescriptor<'_>,
) -> Result<(), MediaReason> {
    producer.validate()?;
    consumer.validate()?;
    (producer == consumer)
        .then_some(())
        .ok_or(MediaReason::DescriptorMismatch)
}

pub fn exact_packet_compatibility(
    producer: PacketDescriptor<'_>,
    consumer: PacketDescriptor<'_>,
) -> Result<(), MediaReason> {
    producer.validate()?;
    consumer.validate()?;
    if producer.codec != consumer.codec
        || producer.profile != consumer.profile
        || producer.extradata_identity != consumer.extradata_identity
    {
        return Err(MediaReason::PacketExtradataMismatch);
    }
    if producer.maximum_bytes != consumer.maximum_bytes
        || producer.time.time_base != consumer.time.time_base
    {
        return Err(MediaReason::DescriptorMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../conformance/c4/media-values.json");
    const AUDIO_PLANES: [Plane; 1] = [Plane {
        offset: 0,
        stride: 8,
        rows: 48,
    }];

    fn audio() -> AudioDescriptor<'static> {
        AudioDescriptor {
            sample_format: "s16le",
            sample_rate_hz: 48_000,
            channel_layout: "stereo-lr",
            channels: 2,
            frames: 192,
            planes: &AUDIO_PLANES,
            maximum_bytes: 384,
        }
    }

    fn time(timestamp: i64) -> MediaTime {
        MediaTime {
            time_base: RationalTimeBase {
                numerator: 1,
                denominator: 48_000,
            },
            timestamp: Some(timestamp),
            duration: 192,
            discontinuity: false,
            conversion_uncertainty_ticks: 1,
        }
    }

    #[test]
    fn synthetic_pcm_is_finite_and_exact() {
        assert_eq!(audio().validate(), Ok(()));
        let mut different = audio();
        different.channel_layout = "stereo-rl";
        assert_eq!(
            exact_audio_compatibility(audio(), different),
            Err(MediaReason::ChannelLayoutMismatch)
        );
    }

    #[test]
    fn time_and_plane_failures_are_explicit() {
        assert_eq!(time(0).validate(), Ok(()));
        let mut oversized = audio();
        oversized.maximum_bytes = 128;
        assert_eq!(oversized.validate(), Err(MediaReason::InvalidPlaneLayout));
    }

    #[test]
    fn stream_metadata_time_and_packet_profiles_are_finite_and_exact() {
        let metadata_entries = [MetadataEntry {
            key: "source",
            value: b"synthetic",
        }];
        let header = MediaValueHeader {
            stream: StreamDescriptor {
                identity: [1; 32],
                time_base: time(0).time_base,
                maximum_frames_per_value: 192,
                maximum_value_bytes: 384,
                maximum_metadata_entries: 1,
                maximum_buffered_values: 2,
            },
            time: time(0),
            clock_correlation: Some(ClockCorrelation {
                media_timestamp: 0,
                host_tick: 10,
                uncertainty_ticks: 1,
            }),
            metadata: MediaMetadata {
                entries: &metadata_entries,
                provenance_identity: [2; 32],
                sensitivity: Sensitivity::Public,
            },
        };
        assert_eq!(header.validate(), Ok(()));
        assert_eq!(
            validate_timestamp_sequence(&[time(0), time(0)]),
            Err(MediaReason::DuplicateTimestamp)
        );

        let packet = PacketDescriptor {
            codec: "pcm-s16le",
            profile: "stereo-48000",
            extradata_identity: [3; 32],
            key: true,
            discontinuity: false,
            time: time(0),
            maximum_bytes: 384,
        };
        let mut incompatible = packet;
        incompatible.extradata_identity = [4; 32];
        assert_eq!(
            exact_packet_compatibility(packet, incompatible),
            Err(MediaReason::PacketExtradataMismatch)
        );
    }

    #[test]
    fn metadata_and_clock_correlation_fail_closed() {
        let oversized = [MetadataEntry {
            key: "source",
            value: &[0; MAXIMUM_METADATA_BYTES + 1],
        }];
        assert_eq!(
            MediaMetadata {
                entries: &oversized,
                provenance_identity: [2; 32],
                sensitivity: Sensitivity::Restricted,
            }
            .validate(),
            Err(MediaReason::MetadataOverflow)
        );

        let header = MediaValueHeader {
            stream: StreamDescriptor {
                identity: [1; 32],
                time_base: time(0).time_base,
                maximum_frames_per_value: 192,
                maximum_value_bytes: 384,
                maximum_metadata_entries: 0,
                maximum_buffered_values: 1,
            },
            time: time(0),
            clock_correlation: Some(ClockCorrelation {
                media_timestamp: 1,
                host_tick: 10,
                uncertainty_ticks: 1,
            }),
            metadata: MediaMetadata {
                entries: &[],
                provenance_identity: [2; 32],
                sensitivity: Sensitivity::Public,
            },
        };
        assert_eq!(header.validate(), Err(MediaReason::IncompatibleTimeBase));
    }

    #[test]
    fn conformance_fixture_owns_the_complete_first_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "conduit.media-value-conformance");
        assert_eq!(fixture["schema_version"], 0);
        assert_eq!(fixture["types"].as_array().unwrap().len(), 6);
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 5);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 14);
    }

    #[test]
    fn semantic_understanding_does_not_install_media_operations() {
        let source = "panel 0\nframe: conduit.media/audio-frame/literal { fixture = \"tone-s16le-stereo-48000\" }\ninspect: conduit.media/audio-frame/inspect\noutput: display/text\nframe.frame > inspect.frame { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }\ninspect.summary > output.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }\n";
        let panel = conduit_panel::parse(source).unwrap();
        let mut registry = Registry::default();
        register_media_contracts(&mut registry);
        registry.resolve_contracts(&panel).unwrap();
        assert!(
            registry
                .installed_providers()
                .iter()
                .all(|provider| { !provider.contract.id.as_str().starts_with("conduit.media/") })
        );
    }
}
