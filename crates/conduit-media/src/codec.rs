//! Exact bounded codec and container operations.
//!
//! These contracts are separate from the media value foundation. The first
//! deterministic provider deliberately supports one content-addressed
//! PCM/WAVE profile and no ambient codec discovery.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

use super::{AUDIO_FRAME_TYPE, AUDIO_SILENCE_VALUE, TEXT_TYPE};

pub const PCM_WAVE_PROFILE_IDENTITY: &str =
    "sha256:4e4cb77e40a559442cbf79888585e9c046117930cd5a99b383eb8743787cb979";
pub const PCM_WAVE_CONTENT_IDENTITY: &str =
    "sha256:fb20432c75da1dc29b674363a3df19e1966f344387423b92366aef4fedbbfd9d";
pub const EMPTY_EXTRADATA_IDENTITY: &str =
    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
pub const PCM_WAVE_BYTES: usize = 812;
pub const PCM_PACKET_BYTES: usize = 852;
pub const PCM_BYTES: usize = 768;
pub const PCM_FRAMES: u32 = 192;

pub const CONTAINER_CHUNK_DESCRIPTOR: &str =
    "conduit.media/container-chunk|0|wave|finite-chunk-sequence|content-exact";
pub const PACKET_DESCRIPTOR: &str =
    "conduit.media/packet|0|codec,profile,extradata|rational-time|finite-bytes";

pub const CONTAINER_CHUNK_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/container-chunk"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xb0, 0x46, 0x9e, 0x58, 0x58, 0x2a, 0x8d, 0x91, 0x48, 0x06, 0xb0, 0x4b, 0x1a, 0xae, 0x22,
        0xff, 0x8a, 0x0c, 0x63, 0xe0, 0xdc, 0x45, 0xfb, 0x27, 0x89, 0x46, 0x55, 0xdf, 0x4b, 0x28,
        0x77, 0x81,
    ]),
};
pub const PACKET_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/packet"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x4e, 0x90, 0x79, 0xa0, 0xd1, 0xba, 0x02, 0x63, 0xfb, 0x17, 0x26, 0xa6, 0x6e, 0x62, 0x24,
        0xb4, 0xca, 0x06, 0x08, 0xb9, 0x43, 0x78, 0x8c, 0x1e, 0x09, 0x09, 0x69, 0x0e, 0x5f, 0x79,
        0x91, 0x80,
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

const CODEC_FIELDS: [ConfigFieldContract<'static>; 14] = [
    field("container", TEXT_TYPE),
    field("codec", TEXT_TYPE),
    field("profile", TEXT_TYPE),
    field("extradata_identity", TEXT_TYPE),
    field("profile_identity", TEXT_TYPE),
    field("maximum_input_bytes", U64_TYPE),
    field("maximum_output_bytes", U64_TYPE),
    field("maximum_tracks", U64_TYPE),
    field("maximum_packets", U64_TYPE),
    field("maximum_reorder_depth", U64_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_metadata_entries", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("flush", TEXT_TYPE),
];
const CODEC_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &CODEC_FIELDS,
};
const WAVE_LITERAL_FIELDS: [ConfigFieldContract<'static>; 3] = [
    field("fixture", TEXT_TYPE),
    field("content_identity", TEXT_TYPE),
    field("maximum_output_bytes", U64_TYPE),
];
const WAVE_LITERAL_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &WAVE_LITERAL_FIELDS,
};

const fn port(
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

const CONTAINER_INPUT: [PortContract<'static>; 1] = [port(
    "container",
    Direction::Input,
    CONTAINER_CHUNK_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const CONTAINER_OUTPUT: [PortContract<'static>; 1] = [port(
    "container",
    Direction::Output,
    CONTAINER_CHUNK_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const PACKET_INPUT: [PortContract<'static>; 1] = [port(
    "packet",
    Direction::Input,
    PACKET_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const PACKET_OUTPUT: [PortContract<'static>; 1] = [port(
    "packet",
    Direction::Output,
    PACKET_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const AUDIO_INPUT: [PortContract<'static>; 1] = [port(
    "frame",
    Direction::Input,
    AUDIO_FRAME_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const AUDIO_OUTPUT: [PortContract<'static>; 1] = [port(
    "frame",
    Direction::Output,
    AUDIO_FRAME_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const PROBE_OUTPUT: [PortContract<'static>; 1] = [PortContract {
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

pub const WAVE_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/wave/literal"),
    config: WAVE_LITERAL_CONFIG,
    inputs: &[],
    outputs: &CONTAINER_OUTPUT,
};
pub const PROBE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/container/probe"),
    config: CODEC_CONFIG,
    inputs: &CONTAINER_INPUT,
    outputs: &PROBE_OUTPUT,
};
pub const DEMUX_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/container/demux"),
    config: CODEC_CONFIG,
    inputs: &CONTAINER_INPUT,
    outputs: &PACKET_OUTPUT,
};
pub const MUX_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/container/mux"),
    config: CODEC_CONFIG,
    inputs: &PACKET_INPUT,
    outputs: &CONTAINER_OUTPUT,
};
pub const DECODE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/decode"),
    config: CODEC_CONFIG,
    inputs: &PACKET_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const ENCODE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/encode"),
    config: CODEC_CONFIG,
    inputs: &AUDIO_INPUT,
    outputs: &PACKET_OUTPUT,
};

pub const CODEC_CONTRACTS: [&NodeContract<'static>; 6] = [
    &WAVE_LITERAL_CONTRACT,
    &PROBE_CONTRACT,
    &DEMUX_CONTRACT,
    &MUX_CONTRACT,
    &DECODE_CONTRACT,
    &ENCODE_CONTRACT,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecReason {
    WrongType,
    UnsupportedBinding,
    InputOverflow,
    OutputOverflow,
    RetainedBufferOverflow,
    WorkOverflow,
    TruncatedContainer,
    MalformedContainer,
    WrongCodec,
    WrongProfile,
    ExtradataMismatch,
    PacketOverflow,
    ReorderedTimestamp,
    FlushRequired,
    Cancelled,
}

impl CodecReason {
    fn code(self) -> &'static str {
        match self {
            Self::WrongType => "CND-CODEC-001",
            Self::UnsupportedBinding | Self::WrongCodec | Self::WrongProfile => "CND-CODEC-002",
            Self::InputOverflow
            | Self::OutputOverflow
            | Self::RetainedBufferOverflow
            | Self::PacketOverflow => "CND-CODEC-003",
            Self::WorkOverflow => "CND-CODEC-004",
            Self::TruncatedContainer => "CND-CODEC-005",
            Self::MalformedContainer => "CND-CODEC-006",
            Self::ExtradataMismatch => "CND-CODEC-007",
            Self::ReorderedTimestamp => "CND-CODEC-008",
            Self::FlushRequired => "CND-CODEC-009",
            Self::Cancelled => "CND-CODEC-010",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodecBounds {
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_tracks: usize,
    pub maximum_packets: usize,
    pub maximum_reorder_depth: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_metadata_entries: usize,
    pub maximum_work: usize,
}

impl CodecBounds {
    pub const FIRST_PROOF: Self = Self {
        maximum_input_bytes: 1024,
        maximum_output_bytes: 1024,
        maximum_tracks: 1,
        maximum_packets: 1,
        maximum_reorder_depth: 0,
        maximum_retained_bytes: 1024,
        maximum_metadata_entries: 0,
        maximum_work: 4096,
    };

    fn validate(self) -> Result<(), CodecReason> {
        if self.maximum_input_bytes == 0
            || self.maximum_output_bytes == 0
            || self.maximum_tracks != 1
            || self.maximum_packets != 1
            || self.maximum_reorder_depth != 0
            || self.maximum_retained_bytes == 0
            || self.maximum_metadata_entries != 0
            || self.maximum_work < self.maximum_input_bytes
        {
            return Err(CodecReason::UnsupportedBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedWave {
    pub pcm: Vec<u8>,
    pub frames: u32,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub metadata_entries: usize,
    pub work: usize,
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, CodecReason> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(CodecReason::TruncatedContainer)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CodecReason> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(CodecReason::TruncatedContainer)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

pub fn pcm_wave_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PCM_WAVE_BYTES);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&804_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&48_000_u32.to_le_bytes());
    bytes.extend_from_slice(&192_000_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&768_u32.to_le_bytes());
    bytes.resize(PCM_WAVE_BYTES, 0);
    bytes
}

pub fn normalize_wave_chunks(
    chunks: &[&[u8]],
    bounds: CodecBounds,
) -> Result<NormalizedWave, CodecReason> {
    bounds.validate()?;
    let total = chunks.iter().try_fold(0_usize, |total, chunk| {
        total
            .checked_add(chunk.len())
            .ok_or(CodecReason::InputOverflow)
    })?;
    if total > bounds.maximum_input_bytes || total > bounds.maximum_retained_bytes {
        return Err(if total > bounds.maximum_input_bytes {
            CodecReason::InputOverflow
        } else {
            CodecReason::RetainedBufferOverflow
        });
    }
    if total > bounds.maximum_work {
        return Err(CodecReason::WorkOverflow);
    }
    let mut bytes = Vec::with_capacity(total);
    for chunk in chunks {
        bytes.extend_from_slice(chunk);
    }
    if bytes.len() < 44 {
        return Err(CodecReason::TruncatedContainer);
    }
    if &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || read_u32(&bytes, 16)? != 16
        || read_u16(&bytes, 20)? != 1
        || read_u16(&bytes, 22)? != 2
        || read_u32(&bytes, 24)? != 48_000
        || read_u32(&bytes, 28)? != 192_000
        || read_u16(&bytes, 32)? != 4
        || read_u16(&bytes, 34)? != 16
        || &bytes[36..40] != b"data"
    {
        return Err(CodecReason::MalformedContainer);
    }
    let riff_size =
        usize::try_from(read_u32(&bytes, 4)?).map_err(|_| CodecReason::InputOverflow)?;
    let data_size =
        usize::try_from(read_u32(&bytes, 40)?).map_err(|_| CodecReason::InputOverflow)?;
    if riff_size + 8 != bytes.len() || data_size + 44 != bytes.len() {
        return Err(
            if riff_size + 8 > bytes.len() || data_size + 44 > bytes.len() {
                CodecReason::TruncatedContainer
            } else {
                CodecReason::MalformedContainer
            },
        );
    }
    if data_size != PCM_BYTES || bytes.len() != PCM_WAVE_BYTES {
        return Err(CodecReason::WrongProfile);
    }
    Ok(NormalizedWave {
        pcm: bytes[44..].to_vec(),
        frames: PCM_FRAMES,
        sample_rate_hz: 48_000,
        channels: 2,
        metadata_entries: 0,
        work: total,
    })
}

fn packet_bytes(pcm: &[u8], bounds: CodecBounds) -> Result<Vec<u8>, CodecReason> {
    bounds.validate()?;
    if pcm.len() != PCM_BYTES || bounds.maximum_packets < 1 {
        return Err(CodecReason::WrongProfile);
    }
    let mut packet = Vec::with_capacity(PCM_PACKET_BYTES);
    packet.extend_from_slice(b"CMP0");
    packet.extend_from_slice(&[
        0x4e, 0x4c, 0xb7, 0x7e, 0x40, 0xa5, 0x59, 0x44, 0x2c, 0xbf, 0x79, 0x88, 0x85, 0x85, 0xe9,
        0xc0, 0x46, 0x11, 0x79, 0x30, 0xcd, 0x5a, 0x99, 0xb3, 0x83, 0xeb, 0x87, 0x43, 0x78, 0x7c,
        0xb9, 0x79,
    ]);
    packet.extend_from_slice(&[
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ]);
    packet.extend_from_slice(&0_i64.to_le_bytes());
    packet.extend_from_slice(&PCM_FRAMES.to_le_bytes());
    packet.extend_from_slice(&(PCM_BYTES as u32).to_le_bytes());
    packet.extend_from_slice(pcm);
    if packet.len() > bounds.maximum_output_bytes {
        return Err(CodecReason::OutputOverflow);
    }
    Ok(packet)
}

fn packet_pcm(packet: &[u8], bounds: CodecBounds) -> Result<&[u8], CodecReason> {
    bounds.validate()?;
    if packet.len() > bounds.maximum_input_bytes {
        return Err(CodecReason::InputOverflow);
    }
    if packet.len() > bounds.maximum_retained_bytes {
        return Err(CodecReason::RetainedBufferOverflow);
    }
    if packet.len() > bounds.maximum_work {
        return Err(CodecReason::WorkOverflow);
    }
    if packet.len() != PCM_PACKET_BYTES || packet.get(0..4) != Some(b"CMP0") {
        return Err(CodecReason::WrongCodec);
    }
    let profile = packet.get(4..36).ok_or(CodecReason::PacketOverflow)?;
    if profile
        != [
            0x4e, 0x4c, 0xb7, 0x7e, 0x40, 0xa5, 0x59, 0x44, 0x2c, 0xbf, 0x79, 0x88, 0x85, 0x85,
            0xe9, 0xc0, 0x46, 0x11, 0x79, 0x30, 0xcd, 0x5a, 0x99, 0xb3, 0x83, 0xeb, 0x87, 0x43,
            0x78, 0x7c, 0xb9, 0x79,
        ]
    {
        return Err(CodecReason::WrongProfile);
    }
    if packet.get(36..68)
        != Some(&[
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ])
    {
        return Err(CodecReason::ExtradataMismatch);
    }
    let timestamp = i64::from_le_bytes(packet[68..76].try_into().expect("fixed packet timestamp"));
    let duration = u32::from_le_bytes(packet[76..80].try_into().expect("fixed packet duration"));
    let length = u32::from_le_bytes(packet[80..84].try_into().expect("fixed packet length"));
    if timestamp != 0 {
        return Err(CodecReason::ReorderedTimestamp);
    }
    if duration != PCM_FRAMES || length as usize != PCM_BYTES {
        return Err(CodecReason::WrongProfile);
    }
    Ok(&packet[84..])
}

pub fn demux_wave(chunks: &[&[u8]], bounds: CodecBounds) -> Result<Vec<u8>, CodecReason> {
    let normalized = normalize_wave_chunks(chunks, bounds)?;
    if normalized
        .work
        .checked_add(PCM_PACKET_BYTES)
        .is_none_or(|work| work > bounds.maximum_work)
    {
        return Err(CodecReason::WorkOverflow);
    }
    packet_bytes(&normalized.pcm, bounds)
}

pub fn mux_wave(packet: &[u8], bounds: CodecBounds) -> Result<Vec<u8>, CodecReason> {
    let pcm = packet_pcm(packet, bounds)?;
    if packet
        .len()
        .checked_add(PCM_WAVE_BYTES)
        .is_none_or(|work| work > bounds.maximum_work)
    {
        return Err(CodecReason::WorkOverflow);
    }
    if PCM_WAVE_BYTES > bounds.maximum_output_bytes {
        return Err(CodecReason::OutputOverflow);
    }
    let mut wave = pcm_wave_bytes();
    wave[44..].copy_from_slice(pcm);
    Ok(wave)
}

pub fn encode_pcm_frame(frame: &[u8], bounds: CodecBounds) -> Result<Vec<u8>, CodecReason> {
    bounds.validate()?;
    if frame != AUDIO_SILENCE_VALUE {
        return Err(CodecReason::WrongProfile);
    }
    if frame
        .len()
        .checked_add(PCM_PACKET_BYTES)
        .is_none_or(|work| work > bounds.maximum_work)
    {
        return Err(CodecReason::WorkOverflow);
    }
    packet_bytes(&[0; PCM_BYTES], bounds)
}

pub fn decode_pcm_packet(packet: &[u8], bounds: CodecBounds) -> Result<Vec<u8>, CodecReason> {
    let pcm = packet_pcm(packet, bounds)?;
    if packet
        .len()
        .checked_add(AUDIO_SILENCE_VALUE.len())
        .is_none_or(|work| work > bounds.maximum_work)
    {
        return Err(CodecReason::WorkOverflow);
    }
    if pcm.iter().any(|byte| *byte != 0) {
        return Err(CodecReason::WrongProfile);
    }
    if AUDIO_SILENCE_VALUE.len() > bounds.maximum_output_bytes {
        return Err(CodecReason::OutputOverflow);
    }
    Ok(AUDIO_SILENCE_VALUE.to_vec())
}

fn exact_u64(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn validate_codec_config(node: &Node) -> Result<(), ResolutionError> {
    let exact = node.config.len() == CODEC_FIELDS.len()
        && node.config("container") == Some("wave")
        && node.config("codec") == Some("pcm-s16le")
        && node.config("profile") == Some("stereo-48000-192")
        && node.config("extradata_identity") == Some(EMPTY_EXTRADATA_IDENTITY)
        && node.config("profile_identity") == Some(PCM_WAVE_PROFILE_IDENTITY)
        && exact_u64(node, "maximum_input_bytes") == Some(1024)
        && exact_u64(node, "maximum_output_bytes") == Some(1024)
        && exact_u64(node, "maximum_tracks") == Some(1)
        && exact_u64(node, "maximum_packets") == Some(1)
        && exact_u64(node, "maximum_reorder_depth") == Some(0)
        && exact_u64(node, "maximum_retained_bytes") == Some(1024)
        && exact_u64(node, "maximum_metadata_entries") == Some(0)
        && exact_u64(node, "maximum_work") == Some(4096)
        && node.config("flush") == Some("exact-terminal");
    exact.then_some(()).ok_or_else(|| {
        ResolutionError::new(
            "CND-CODEC-002",
            "codec operation requires the exact bounded PCM/WAVE profile",
        )
    })
}

fn validate_wave_literal(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == WAVE_LITERAL_FIELDS.len()
        && node.config("fixture") == Some("silence-pcm-s16le-stereo-48000-192")
        && node.config("content_identity") == Some(PCM_WAVE_CONTENT_IDENTITY)
        && exact_u64(node, "maximum_output_bytes") == Some(1024))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-CODEC-002",
            "WAVE literal requires the exact content-addressed fixture",
        )
    })
}

fn runtime(reason: CodecReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("PCM/WAVE operation failed: {reason:?}"),
    )
}

fn one_input<'a>(
    inputs: &'a [Value],
    value_type: TypeContractRef<'static>,
) -> Result<&'a [u8], RuntimeError> {
    let [input] = inputs else {
        return Err(runtime(CodecReason::WrongType));
    };
    if input.value_type != value_type {
        return Err(runtime(CodecReason::WrongType));
    }
    Ok(&input.bytes)
}

struct WaveLiteral;
impl Handler for WaveLiteral {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(CodecReason::WrongType));
        }
        Ok(vec![Value {
            value_type: CONTAINER_CHUNK_TYPE,
            bytes: pcm_wave_bytes(),
        }])
    }
}

struct Probe;
impl Handler for Probe {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let bytes = one_input(inputs, CONTAINER_CHUNK_TYPE)?;
        let wave = normalize_wave_chunks(&[bytes], CodecBounds::FIRST_PROOF).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "wave:pcm-s16le:{}:{}:{}-track:{}-frames:{}-bytes",
                wave.sample_rate_hz, wave.channels, 1, wave.frames, PCM_WAVE_BYTES
            )
            .into_bytes(),
        }])
    }
}

struct Demux;
impl Handler for Demux {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let bytes = one_input(inputs, CONTAINER_CHUNK_TYPE)?;
        Ok(vec![Value {
            value_type: PACKET_TYPE,
            bytes: demux_wave(&[bytes], CodecBounds::FIRST_PROOF).map_err(runtime)?,
        }])
    }
}

struct Mux;
impl Handler for Mux {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let bytes = one_input(inputs, PACKET_TYPE)?;
        Ok(vec![Value {
            value_type: CONTAINER_CHUNK_TYPE,
            bytes: mux_wave(bytes, CodecBounds::FIRST_PROOF).map_err(runtime)?,
        }])
    }
}

struct Decode;
impl Handler for Decode {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let bytes = one_input(inputs, PACKET_TYPE)?;
        Ok(vec![Value {
            value_type: AUDIO_FRAME_TYPE,
            bytes: decode_pcm_packet(bytes, CodecBounds::FIRST_PROOF).map_err(runtime)?,
        }])
    }
}

struct Encode;
impl Handler for Encode {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let bytes = one_input(inputs, AUDIO_FRAME_TYPE)?;
        Ok(vec![Value {
            value_type: PACKET_TYPE,
            bytes: encode_pcm_frame(bytes, CodecBounds::FIRST_PROOF).map_err(runtime)?,
        }])
    }
}

pub fn register_media_codec_contracts(registry: &mut Registry) {
    for contract in CODEC_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_media_codec_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &WAVE_LITERAL_CONTRACT,
            "conduit.media/wave-literal-deterministic",
            "conduit.media/wave-literal-artifact",
            "media-wave-literal",
            (|| Box::new(WaveLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_wave_literal as conduit_runtime::ConfigValidator,
        ),
        (
            &PROBE_CONTRACT,
            "conduit.media/wave-probe-deterministic",
            "conduit.media/wave-probe-artifact",
            "media-wave-probe",
            (|| Box::new(Probe) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_codec_config as conduit_runtime::ConfigValidator,
        ),
        (
            &DEMUX_CONTRACT,
            "conduit.media/wave-demux-deterministic",
            "conduit.media/wave-demux-artifact",
            "media-wave-demux",
            (|| Box::new(Demux) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_codec_config as conduit_runtime::ConfigValidator,
        ),
        (
            &MUX_CONTRACT,
            "conduit.media/wave-mux-deterministic",
            "conduit.media/wave-mux-artifact",
            "media-wave-mux",
            (|| Box::new(Mux) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_codec_config as conduit_runtime::ConfigValidator,
        ),
        (
            &DECODE_CONTRACT,
            "conduit.media/pcm-decode-deterministic",
            "conduit.media/pcm-decode-artifact",
            "media-pcm-decode",
            (|| Box::new(Decode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_codec_config as conduit_runtime::ConfigValidator,
        ),
        (
            &ENCODE_CONTRACT,
            "conduit.media/pcm-encode-deterministic",
            "conduit.media/pcm-encode-artifact",
            "media-pcm-encode",
            (|| Box::new(Encode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_codec_config as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("codec.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config: validator,
        })?;
    }
    Ok(())
}

fn register_codec_provider_set(
    registry: &mut Registry,
    providers: &[(
        &'static NodeContract<'static>,
        &'static str,
        &'static str,
        &'static str,
        conduit_runtime::HandlerFactory,
        conduit_runtime::ConfigValidator,
    )],
) -> Result<(), RegistryError> {
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (
        contract,
        implementation_id,
        artifact_id,
        entrypoint,
        factory,
        validator,
    ) in providers
    {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("codec.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: *factory,
            validate_config: *validator,
        })?;
    }
    Ok(())
}

/// Installs an explicit FFmpeg-style profile over the same published media
/// codec contracts. Contracts and handlers are unchanged; only provider identity
/// and artifact entrypoints differ.
pub fn register_ffmpeg_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_provider_set(
        registry,
        &[
            (
                &PROBE_CONTRACT,
                "conduit.media/wave-probe-ffmpeg",
                "conduit.media/wave-probe-ffmpeg-artifact",
                "media-ffmpeg-wave-probe",
                (|| Box::new(Probe) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &DEMUX_CONTRACT,
                "conduit.media/wave-demux-ffmpeg",
                "conduit.media/wave-demux-ffmpeg-artifact",
                "media-ffmpeg-wave-demux",
                (|| Box::new(Demux) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &MUX_CONTRACT,
                "conduit.media/wave-mux-ffmpeg",
                "conduit.media/wave-mux-ffmpeg-artifact",
                "media-ffmpeg-wave-mux",
                (|| Box::new(Mux) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &DECODE_CONTRACT,
                "conduit.media/pcm-decode-ffmpeg",
                "conduit.media/pcm-decode-ffmpeg-artifact",
                "media-ffmpeg-pcm-decode",
                (|| Box::new(Decode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &ENCODE_CONTRACT,
                "conduit.media/pcm-encode-ffmpeg",
                "conduit.media/pcm-encode-ffmpeg-artifact",
                "media-ffmpeg-pcm-encode",
                (|| Box::new(Encode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
        ],
    )
}

/// Installs an explicit SoX-style profile over the same published media codec
/// contracts where SoX overlap exists.
pub fn register_sox_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_provider_set(
        registry,
        &[
            (
                &DECODE_CONTRACT,
                "conduit.media/pcm-decode-sox",
                "conduit.media/pcm-decode-sox-artifact",
                "media-sox-pcm-decode",
                (|| Box::new(Decode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &ENCODE_CONTRACT,
                "conduit.media/pcm-encode-sox",
                "conduit.media/pcm-encode-sox-artifact",
                "media-sox-pcm-encode",
                (|| Box::new(Encode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
        ],
    )
}

/// Installs a bounded browser-focused media profile with distinct implementation
/// identities.
pub fn register_browser_codec_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_codec_provider_set(
        registry,
        &[
            (
                &DECODE_CONTRACT,
                "conduit.media/pcm-decode-browser",
                "conduit.media/pcm-decode-browser-artifact",
                "media-browser-pcm-decode",
                (|| Box::new(Decode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &ENCODE_CONTRACT,
                "conduit.media/pcm-encode-browser",
                "conduit.media/pcm-encode-browser-artifact",
                "media-browser-pcm-encode",
                (|| Box::new(Encode) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
            (
                &PROBE_CONTRACT,
                "conduit.media/wave-probe-browser",
                "conduit.media/wave-probe-browser-artifact",
                "media-browser-wave-probe",
                (|| Box::new(Probe) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
                validate_codec_config as conduit_runtime::ConfigValidator,
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../conformance/c4/media-codecs.json");

    #[test]
    fn fragmented_and_coalesced_wave_normalize_identically() {
        let wave = pcm_wave_bytes();
        let coalesced = normalize_wave_chunks(&[&wave], CodecBounds::FIRST_PROOF).unwrap();
        let fragmented = normalize_wave_chunks(
            &[
                &wave[..1],
                &wave[1..17],
                &wave[17..43],
                &wave[43..44],
                &wave[44..],
            ],
            CodecBounds::FIRST_PROOF,
        )
        .unwrap();
        assert_eq!(fragmented, coalesced);
        assert_eq!(fragmented.frames, PCM_FRAMES);
        assert_eq!(fragmented.work, PCM_WAVE_BYTES);
    }

    #[test]
    fn exact_round_trip_preserves_the_fixed_pcm_profile() {
        let packet = encode_pcm_frame(AUDIO_SILENCE_VALUE, CodecBounds::FIRST_PROOF).unwrap();
        assert_eq!(packet.len(), PCM_PACKET_BYTES);
        let wave = mux_wave(&packet, CodecBounds::FIRST_PROOF).unwrap();
        assert_eq!(wave, pcm_wave_bytes());
        let packet_again = demux_wave(&[&wave], CodecBounds::FIRST_PROOF).unwrap();
        assert_eq!(packet_again, packet);
        assert_eq!(
            decode_pcm_packet(&packet, CodecBounds::FIRST_PROOF).unwrap(),
            AUDIO_SILENCE_VALUE
        );
    }

    #[test]
    fn malformed_truncated_profile_extradata_reorder_and_bounds_fail_closed() {
        let wave = pcm_wave_bytes();
        assert_eq!(
            normalize_wave_chunks(&[&wave[..43]], CodecBounds::FIRST_PROOF),
            Err(CodecReason::TruncatedContainer)
        );
        let mut malformed = wave.clone();
        malformed[20] = 3;
        assert_eq!(
            normalize_wave_chunks(&[&malformed], CodecBounds::FIRST_PROOF),
            Err(CodecReason::MalformedContainer)
        );

        let packet = demux_wave(&[&wave], CodecBounds::FIRST_PROOF).unwrap();
        let mut wrong_profile = packet.clone();
        wrong_profile[4] ^= 1;
        assert_eq!(
            decode_pcm_packet(&wrong_profile, CodecBounds::FIRST_PROOF),
            Err(CodecReason::WrongProfile)
        );
        let mut wrong_extradata = packet.clone();
        wrong_extradata[36] ^= 1;
        assert_eq!(
            decode_pcm_packet(&wrong_extradata, CodecBounds::FIRST_PROOF),
            Err(CodecReason::ExtradataMismatch)
        );
        let mut reordered = packet.clone();
        reordered[68] = 1;
        assert_eq!(
            decode_pcm_packet(&reordered, CodecBounds::FIRST_PROOF),
            Err(CodecReason::ReorderedTimestamp)
        );

        let too_small = CodecBounds {
            maximum_output_bytes: 64,
            ..CodecBounds::FIRST_PROOF
        };
        assert_eq!(
            demux_wave(&[&wave], too_small),
            Err(CodecReason::OutputOverflow)
        );
        let retained = CodecBounds {
            maximum_retained_bytes: 64,
            ..CodecBounds::FIRST_PROOF
        };
        assert_eq!(
            normalize_wave_chunks(&[&wave], retained),
            Err(CodecReason::RetainedBufferOverflow)
        );
    }

    #[test]
    fn media_values_and_codec_contracts_do_not_install_codec_providers() {
        let mut registry = Registry::default();
        super::super::register_media_contracts(&mut registry);
        register_media_codec_contracts(&mut registry);
        assert!(
            registry
                .installed_providers()
                .iter()
                .all(|provider| { !CODEC_CONTRACTS.contains(&provider.contract) })
        );
    }

    #[test]
    fn conformance_fixture_names_the_complete_first_codec_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "conduit.media-codec-conformance");
        assert_eq!(fixture["schema_version"], 0);
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 9);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 15);
        assert_eq!(fixture["profile_identity"], PCM_WAVE_PROFILE_IDENTITY);
        assert_eq!(
            format!("sha256:{:x}", Sha256::digest(pcm_wave_bytes())),
            PCM_WAVE_CONTENT_IDENTITY
        );
    }
}
