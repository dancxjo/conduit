//! Finite iRobot Create Open Interface device protocol.
//!
//! This module is deliberately below portable robotics meaning. It contains no
//! serial path, GPIO number, Host identity, or Conduit LINE behavior. A Pico W
//! or std provider may implement [`CreateUartProvider`] only after its exact
//! physical Base and UART profile have been admitted.

const START_OPCODE: u8 = 128;
const SAFE_OPCODE: u8 = 131;
const FULL_OPCODE: u8 = 132;
const DRIVE_DIRECT_OPCODE: u8 = 145;
const LEDS_OPCODE: u8 = 139;
const SEEK_DOCK_OPCODE: u8 = 143;
const STREAM_OPCODE: u8 = 148;
const PAUSE_STREAM_OPCODE: u8 = 150;
const SENSORS_OPCODE: u8 = 142;
pub const STREAM_HEADER: u8 = 19;

/// Pete's installed base is the original Create 1, whose protocol is the
/// iRobot Create Open Interface v2 contract, not the later Create 2 OI.
pub const CREATE_1_OI_SPECIFICATION: &str = "iRobot Create Open Interface v2";
pub const CREATE_1_OI_SPECIFICATION_URL: &str =
    "https://ptolemy.berkeley.edu/projects/chess/eecs124/iRobotDocs/CreateOpenInterface_v2.pdf";
pub const CREATE_1_OI_PROTOCOL_VERSION: u8 = 2;
pub const CREATE_1_PLAY_LED_MASK: u8 = 1 << 1;
pub const CREATE_1_ADVANCE_LED_MASK: u8 = 1 << 3;
pub const CREATE_1_LED_MASK: u8 = CREATE_1_PLAY_LED_MASK | CREATE_1_ADVANCE_LED_MASK;

pub const CREATE_OI_BAUD: u32 = 57_600;
pub const CREATE_OI_ALTERNATE_BAUD: u32 = 19_200;
pub const CREATE_OI_MAX_PACKET_BYTES: usize = 26;
pub const CREATE_OI_MAX_FRAME_BYTES: usize = CREATE_OI_MAX_PACKET_BYTES + 4;
pub const CREATE_OI_MAX_COMMAND_BYTES: usize = 5;
pub const CREATE_OI_MAX_WHEEL_SPEED_MM_S: i16 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartProfile {
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: UartParity,
}

impl UartProfile {
    pub const CREATE_OI: Self = Self {
        baud: CREATE_OI_BAUD,
        data_bits: 8,
        stop_bits: 1,
        parity: UartParity::None,
    };

    pub const CREATE_OI_19200: Self = Self {
        baud: CREATE_OI_ALTERNATE_BAUD,
        data_bits: 8,
        stop_bits: 1,
        parity: UartParity::None,
    };

    pub const fn is_create_oi(self) -> bool {
        (self.baud == CREATE_OI_BAUD || self.baud == CREATE_OI_ALTERNATE_BAUD)
            && self.data_bits == 8
            && self.stop_bits == 1
            && matches!(self.parity, UartParity::None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UartParity {
    None,
    Even,
    Odd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOiFailure {
    ProviderUnavailable,
    WrongUartProfile { observed: UartProfile },
    WriteFailed,
    ReadFailed,
    Timeout,
    DeviceNoResponse,
    UnsupportedPacket(u8),
    TruncatedFrame,
    MalformedFrame,
    SynchronizationLimit { maximum_discarded_bytes: u16 },
}

/// Exact physical UART provider boundary. Implementations must not retry or
/// silently change profiles; policy and evidence remain above this seam.
pub trait CreateUartProvider {
    type Error;

    fn is_available(&self) -> bool;
    fn profile(&self) -> UartProfile;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
    fn read_byte(&mut self, deadline_tick: u64) -> Result<Option<u8>, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodedOiCommand {
    bytes: [u8; CREATE_OI_MAX_COMMAND_BYTES],
    len: u8,
}

impl EncodedOiCommand {
    const fn one(byte: u8) -> Self {
        Self {
            bytes: [byte, 0, 0, 0, 0],
            len: 1,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOiModeRequest {
    Passive,
    Safe,
    Full,
}

pub fn encode_start() -> EncodedOiCommand {
    EncodedOiCommand::one(START_OPCODE)
}

pub fn encode_mode(mode: CreateOiModeRequest) -> Option<EncodedOiCommand> {
    match mode {
        CreateOiModeRequest::Passive => None,
        CreateOiModeRequest::Safe => Some(EncodedOiCommand::one(SAFE_OPCODE)),
        CreateOiModeRequest::Full => Some(EncodedOiCommand::one(FULL_OPCODE)),
    }
}

pub fn encode_drive_direct(
    left_mm_s: i16,
    right_mm_s: i16,
) -> Result<EncodedOiCommand, CreateOiFailure> {
    if left_mm_s.unsigned_abs() > CREATE_OI_MAX_WHEEL_SPEED_MM_S as u16
        || right_mm_s.unsigned_abs() > CREATE_OI_MAX_WHEEL_SPEED_MM_S as u16
    {
        return Err(CreateOiFailure::MalformedFrame);
    }
    let left = left_mm_s.to_be_bytes();
    let right = right_mm_s.to_be_bytes();
    Ok(EncodedOiCommand {
        bytes: [DRIVE_DIRECT_OPCODE, right[0], right[1], left[0], left[1]],
        len: 5,
    })
}

pub fn encode_stop() -> EncodedOiCommand {
    encode_drive_direct(0, 0).expect("zero wheel speed is valid")
}

pub fn encode_seek_dock() -> EncodedOiCommand {
    EncodedOiCommand::one(SEEK_DOCK_OPCODE)
}

pub fn encode_lights(led_bits: u8, color: u8, intensity: u8) -> EncodedOiCommand {
    EncodedOiCommand {
        // Create 1 OI v2 assigns only bit 1 to PLAY and bit 3 to ADVANCE.
        // Bits 0 and 2 are not Create 1 LED outputs and must not leak from a
        // later Create/Roomba LED layout into this codec.
        bytes: [
            LEDS_OPCODE,
            led_bits & CREATE_1_LED_MASK,
            color,
            intensity,
            0,
        ],
        len: 4,
    }
}

/// Request one allow-listed Create OI sensor packet. Unlike a stream frame,
/// the device replies with only the packet payload bytes.
pub fn encode_query_sensor(packet_id: u8) -> Result<EncodedOiCommand, CreateOiFailure> {
    sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
    Ok(EncodedOiCommand {
        bytes: [SENSORS_OPCODE, packet_id, 0, 0, 0],
        len: 2,
    })
}

pub fn encode_sensor_stream(packet_id: u8) -> Result<EncodedOiCommand, CreateOiFailure> {
    sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
    Ok(EncodedOiCommand {
        bytes: [STREAM_OPCODE, 1, packet_id, 0, 0],
        len: 3,
    })
}

/// Encode the exact finite two-packet stream used for correlated observations.
pub fn encode_sensor_stream_pair(
    first_packet_id: u8,
    second_packet_id: u8,
) -> Result<EncodedOiCommand, CreateOiFailure> {
    sensor_packet_len(first_packet_id)
        .ok_or(CreateOiFailure::UnsupportedPacket(first_packet_id))?;
    sensor_packet_len(second_packet_id)
        .ok_or(CreateOiFailure::UnsupportedPacket(second_packet_id))?;
    Ok(EncodedOiCommand {
        bytes: [STREAM_OPCODE, 2, first_packet_id, second_packet_id, 0],
        len: 4,
    })
}

/// Encode the finite three-packet stream used by the embedded Create
/// supervisor. Its callers choose only allow-listed packet identities.
pub fn encode_sensor_stream_triplet(
    first_packet_id: u8,
    second_packet_id: u8,
    third_packet_id: u8,
) -> Result<EncodedOiCommand, CreateOiFailure> {
    sensor_packet_len(first_packet_id)
        .ok_or(CreateOiFailure::UnsupportedPacket(first_packet_id))?;
    sensor_packet_len(second_packet_id)
        .ok_or(CreateOiFailure::UnsupportedPacket(second_packet_id))?;
    sensor_packet_len(third_packet_id)
        .ok_or(CreateOiFailure::UnsupportedPacket(third_packet_id))?;
    Ok(EncodedOiCommand {
        bytes: [
            STREAM_OPCODE,
            3,
            first_packet_id,
            second_packet_id,
            third_packet_id,
        ],
        len: 5,
    })
}

pub fn encode_pause_stream() -> EncodedOiCommand {
    EncodedOiCommand {
        bytes: [PAUSE_STREAM_OPCODE, 0, 0, 0, 0],
        len: 2,
    }
}

pub fn write_command<P: CreateUartProvider>(
    provider: &mut P,
    command: &EncodedOiCommand,
) -> Result<(), CreateOiFailure> {
    require_provider(provider)?;
    provider
        .write_all(command.as_bytes())
        .map_err(|_| CreateOiFailure::WriteFailed)
}

pub fn require_provider<P: CreateUartProvider>(provider: &P) -> Result<(), CreateOiFailure> {
    if !provider.is_available() {
        return Err(CreateOiFailure::ProviderUnavailable);
    }
    let observed = provider.profile();
    if !observed.is_create_oi() {
        return Err(CreateOiFailure::WrongUartProfile { observed });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateOiPacket {
    pub packet_id: u8,
    bytes: [u8; CREATE_OI_MAX_PACKET_BYTES],
    len: u8,
}

impl CreateOiPacket {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    fn checked(packet_id: u8, payload: &[u8]) -> Result<Self, CreateOiFailure> {
        let expected =
            sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
        if payload.len() != expected {
            return Err(CreateOiFailure::MalformedFrame);
        }
        validate_sensor_payload(packet_id, payload)?;
        let mut bytes = [0_u8; CREATE_OI_MAX_PACKET_BYTES];
        bytes[..expected].copy_from_slice(payload);
        Ok(Self {
            packet_id,
            bytes,
            len: expected as u8,
        })
    }
}

/// Validate one packet payload already separated from its stream envelope.
pub fn decode_sensor_packet(
    packet_id: u8,
    payload: &[u8],
) -> Result<CreateOiPacket, CreateOiFailure> {
    CreateOiPacket::checked(packet_id, payload)
}

pub fn read_stream_packet<P: CreateUartProvider>(
    provider: &mut P,
    packet_id: u8,
    deadline_tick: u64,
) -> Result<CreateOiPacket, CreateOiFailure> {
    require_provider(provider)?;
    let expected =
        sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
    let mut frame = [0_u8; CREATE_OI_MAX_FRAME_BYTES];
    let frame_len = expected + 4;
    for (index, slot) in frame[..frame_len].iter_mut().enumerate() {
        let byte = provider
            .read_byte(deadline_tick)
            .map_err(|_| CreateOiFailure::ReadFailed)?;
        *slot = match byte {
            Some(byte) => byte,
            None if index == 0 => return Err(CreateOiFailure::DeviceNoResponse),
            None => return Err(CreateOiFailure::TruncatedFrame),
        };
    }
    decode_stream_frame(packet_id, &frame[..frame_len])
}

/// Read one raw payload returned by [`encode_query_sensor`]. The caller owns
/// the finite request deadline and must send exactly one matching query first.
pub fn read_query_sensor_packet<P: CreateUartProvider>(
    provider: &mut P,
    packet_id: u8,
    deadline_tick: u64,
) -> Result<CreateOiPacket, CreateOiFailure> {
    require_provider(provider)?;
    let expected =
        sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
    let mut payload = [0_u8; CREATE_OI_MAX_PACKET_BYTES];
    for (index, slot) in payload[..expected].iter_mut().enumerate() {
        let byte = provider
            .read_byte(deadline_tick)
            .map_err(|_| CreateOiFailure::ReadFailed)?;
        *slot = match byte {
            Some(byte) => byte,
            None if index == 0 => return Err(CreateOiFailure::DeviceNoResponse),
            None => return Err(CreateOiFailure::TruncatedFrame),
        };
    }
    CreateOiPacket::checked(packet_id, &payload[..expected])
}

pub fn decode_stream_frame(packet_id: u8, frame: &[u8]) -> Result<CreateOiPacket, CreateOiFailure> {
    let expected =
        sensor_packet_len(packet_id).ok_or(CreateOiFailure::UnsupportedPacket(packet_id))?;
    if frame.len() < expected + 4 {
        return Err(CreateOiFailure::TruncatedFrame);
    }
    if frame.len() != expected + 4
        || frame[0] != STREAM_HEADER
        || usize::from(frame[1]) != expected + 1
        || frame[2] != packet_id
        || frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) != 0
    {
        return Err(CreateOiFailure::MalformedFrame);
    }
    CreateOiPacket::checked(packet_id, &frame[3..3 + expected])
}

pub fn sensor_packet_len(packet_id: u8) -> Option<usize> {
    match packet_id {
        0 => Some(26),
        1 => Some(10),
        7..=18 | 21 | 24 | 32 | 34..=38 => Some(1),
        19 | 20 | 22 | 23 | 25..=31 => Some(2),
        _ => None,
    }
}

fn validate_sensor_payload(packet_id: u8, bytes: &[u8]) -> Result<(), CreateOiFailure> {
    let valid = match packet_id {
        0 => valid_group_zero(bytes),
        1 => valid_group_one(bytes),
        7 => bytes[0] & !0x1f == 0,
        8..=13 => bytes[0] <= 1,
        // Create 1 exposes Play on bit 0 and Advance on bit 2. Bits 1 and 3
        // are reserved, despite the packet's broad numeric range in the quick
        // reference, and must not make corrupt UART data look valid.
        18 => bytes[0] & !0x05 == 0,
        21 => bytes[0] <= 5,
        34 => bytes[0] & !0x03 == 0,
        35 => bytes[0] <= 3,
        _ => true,
    };
    valid.then_some(()).ok_or(CreateOiFailure::MalformedFrame)
}

fn valid_group_one(bytes: &[u8]) -> bool {
    bytes.len() == 10 && bytes[0] & !0x1f == 0 && bytes[1..=6].iter().all(|value| *value <= 1)
}

fn valid_group_zero(bytes: &[u8]) -> bool {
    bytes.len() == 26
        && bytes[0] & !0x1f == 0
        && bytes[1..=6].iter().all(|value| *value <= 1)
        && bytes[11] & !0x05 == 0
        && bytes[16] <= 5
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
