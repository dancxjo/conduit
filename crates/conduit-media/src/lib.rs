//! Host-neutral bounded media value contracts.
//!
//! This crate defines values and exact compatibility only. It does not expose
//! codecs, devices, host discovery, implicit conversion, or another event
//! model.

use sha2::{Digest, Sha256};

use conduit_core::{
    ConfigContract, ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract,
    PortContract, PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract,
    TerminalContract, TypeContractRef, ValueCardinality,
};
use conduit_panel::Node;
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

pub const MAXIMUM_PLANES: usize = 4;
pub const MAXIMUM_CHANNELS: u16 = 64;
pub const MAXIMUM_METADATA_ENTRIES: u16 = 64;
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
const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
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
const AUDIO_INPUT: [PortContract<'static>; 1] = [port("frame", Direction::Input, AUDIO_FRAME_TYPE)];
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

pub const AUDIO_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio-frame/literal"),
    config: ConfigContract { fields: &[] },
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
    config: ConfigContract { fields: &[] },
    inputs: &[],
    outputs: &VIDEO_OUTPUT,
};
pub const VIDEO_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/video-frame/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &VIDEO_INPUT,
    outputs: &TEXT_OUTPUT,
};

const AUDIO_VALUE: &[u8] = b"CMA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xc0synthetic-pcm";
const VIDEO_VALUE: &[u8] = b"CMV0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01synthetic-rgb";

struct LiteralHandler {
    value_type: TypeContractRef<'static>,
    bytes: &'static [u8],
}

impl Handler for LiteralHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-MEDIA-001",
                "media literal received hidden input",
            ));
        }
        Ok(vec![Value {
            value_type: self.value_type,
            bytes: self.bytes.to_vec(),
        }])
    }
}

struct InspectHandler {
    expected_type: TypeContractRef<'static>,
    magic: &'static [u8; 4],
    summary: &'static [u8],
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
        if input.value_type != self.expected_type
            || input.bytes.len() > 64
            || !input.bytes.starts_with(self.magic)
        {
            return Err(RuntimeError::new(
                "CND-MEDIA-003",
                "media frame representation is invalid",
            ));
        }
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: self.summary.to_vec(),
        }])
    }
}

fn audio_literal() -> Box<dyn Handler> {
    Box::new(LiteralHandler {
        value_type: AUDIO_FRAME_TYPE,
        bytes: AUDIO_VALUE,
    })
}
fn video_literal() -> Box<dyn Handler> {
    Box::new(LiteralHandler {
        value_type: VIDEO_FRAME_TYPE,
        bytes: VIDEO_VALUE,
    })
}
fn audio_inspect() -> Box<dyn Handler> {
    Box::new(InspectHandler {
        expected_type: AUDIO_FRAME_TYPE,
        magic: b"CMA0",
        summary: b"audio:s16le:48000:stereo:192",
    })
}
fn video_inspect() -> Box<dyn Handler> {
    Box::new(InspectHandler {
        expected_type: VIDEO_FRAME_TYPE,
        magic: b"CMV0",
        summary: b"video:rgb24:2x2",
    })
}

fn validate_no_config(node: &Node) -> Result<(), ResolutionError> {
    if node.config.is_empty() {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MEDIA-004",
            "synthetic media fixture has no configuration",
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
    for (contract, implementation_id, artifact_id, entrypoint, factory) in [
        (
            &AUDIO_LITERAL_CONTRACT,
            "conduit.media/audio-literal-deterministic",
            "conduit.media/audio-literal-artifact",
            "media-audio-literal",
            audio_literal as conduit_runtime::HandlerFactory,
        ),
        (
            &AUDIO_INSPECT_CONTRACT,
            "conduit.media/audio-inspect-deterministic",
            "conduit.media/audio-inspect-artifact",
            "media-audio-inspect",
            audio_inspect as conduit_runtime::HandlerFactory,
        ),
        (
            &VIDEO_LITERAL_CONTRACT,
            "conduit.media/video-literal-deterministic",
            "conduit.media/video-literal-artifact",
            "media-video-literal",
            video_literal as conduit_runtime::HandlerFactory,
        ),
        (
            &VIDEO_INSPECT_CONTRACT,
            "conduit.media/video-inspect-deterministic",
            "conduit.media/video-inspect-artifact",
            "media-video-inspect",
            video_inspect as conduit_runtime::HandlerFactory,
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
            validate_config: validate_no_config,
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
    pub maximum_bytes: usize,
}

impl PacketDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
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
        assert_eq!(
            MediaTime {
                time_base: RationalTimeBase {
                    numerator: 1,
                    denominator: 48_000,
                },
                timestamp: Some(0),
                duration: 192,
                discontinuity: false,
                conversion_uncertainty_ticks: 0,
            }
            .validate(),
            Ok(())
        );
        let mut oversized = audio();
        oversized.maximum_bytes = 128;
        assert_eq!(oversized.validate(), Err(MediaReason::InvalidPlaneLayout));
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
}
