//! Exact bounded framing contract for the first simplex infrared Line profile.
//!
//! This module starts and ends at bytes. Carrier generation, pulse timing, and
//! receiver-specific demodulation remain realization work below the generic
//! Line/Cord boundary.

use conduit_core::{
    LineContinuation, LineContract, LineDuplex, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape, LinkLimits,
};

const PREAMBLE: [u8; 4] = *b"CNDI";
const REVISION: u8 = 1;
const PROFILE: u8 = 1;
const HEADER_BYTES: usize = 10;
const INTEGRITY_BYTES: usize = 2;

pub const INFRARED_MAXIMUM_PAYLOAD_BYTES: usize = 2_048;
pub const INFRARED_MAXIMUM_FRAME_BYTES: usize =
    HEADER_BYTES + INFRARED_MAXIMUM_PAYLOAD_BYTES + INTEGRITY_BYTES;

/// Physical requirements an implementation must offer before it can realize
/// this profile. They do not become Form or Cord meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfraredRealizationRequirements {
    pub carrier_hz: u32,
    pub carrier_tolerance_hz: u32,
    pub timing_unit_micros: u16,
    pub inter_frame_gap_micros: u32,
}

/// The only admitted first infrared profile. Each exact Line has one source
/// and one sink through its `LinkBinding`; the reverse path requires another
/// independently admitted Line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfraredSimplexProfile {
    pub revision: u8,
    pub profile: u8,
    pub maximum_payload_bytes: u16,
    pub transmit_queue_items: u8,
    pub receive_queue_items: u8,
    pub requirements: InfraredRealizationRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraredProfileError {
    UnsupportedRevision,
    UnsupportedProfile,
    InvalidPayloadLimit,
    InvalidQueueLimit,
    InvalidCarrier,
    InvalidTiming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraredFrameError {
    EmptyPayload,
    OversizedPayload,
    OutputTooSmall,
    IncompleteFrame,
    InvalidPreamble,
    UnsupportedRevision,
    UnsupportedProfile,
    LengthMismatch,
    IntegrityMismatch,
    DuplicateFrame,
    ReorderedFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfraredFrame<'a> {
    pub sequence: u16,
    pub payload: &'a [u8],
}

impl InfraredSimplexProfile {
    pub const FIRST: Self = Self {
        revision: REVISION,
        profile: PROFILE,
        maximum_payload_bytes: INFRARED_MAXIMUM_PAYLOAD_BYTES as u16,
        transmit_queue_items: 1,
        receive_queue_items: 1,
        requirements: InfraredRealizationRequirements {
            carrier_hz: 38_000,
            carrier_tolerance_hz: 1_000,
            timing_unit_micros: 562,
            inter_frame_gap_micros: 20_000,
        },
    };

    pub fn validate(self) -> Result<Self, InfraredProfileError> {
        if self.revision != REVISION {
            return Err(InfraredProfileError::UnsupportedRevision);
        }
        if self.profile != PROFILE {
            return Err(InfraredProfileError::UnsupportedProfile);
        }
        if self.maximum_payload_bytes == 0
            || usize::from(self.maximum_payload_bytes) > INFRARED_MAXIMUM_PAYLOAD_BYTES
        {
            return Err(InfraredProfileError::InvalidPayloadLimit);
        }
        if self.transmit_queue_items != 1 || self.receive_queue_items != 1 {
            return Err(InfraredProfileError::InvalidQueueLimit);
        }
        if self.requirements.carrier_hz == 0
            || self.requirements.carrier_tolerance_hz >= self.requirements.carrier_hz
        {
            return Err(InfraredProfileError::InvalidCarrier);
        }
        if self.requirements.timing_unit_micros == 0
            || self.requirements.inter_frame_gap_micros
                <= u32::from(self.requirements.timing_unit_micros)
        {
            return Err(InfraredProfileError::InvalidTiming);
        }
        Ok(self)
    }

    pub const fn line_contract() -> LineContract {
        LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::Simplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::BestEffort,
            continuation: LineContinuation::None,
            security: LineSecurity::PhysicalPossession,
        }
    }

    pub fn link_limits(self) -> Result<LinkLimits, InfraredProfileError> {
        let profile = self.validate()?;
        let maximum_frame_bytes = u32::from(profile.maximum_payload_bytes)
            + u32::try_from(HEADER_BYTES + INTEGRITY_BYTES).unwrap_or(u32::MAX);
        Ok(LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: u32::from(profile.maximum_payload_bytes),
            maximum_buffered_bytes: maximum_frame_bytes.saturating_mul(2),
            maximum_frame_bytes,
        })
    }
}

pub fn encode_infrared_frame(
    payload: &[u8],
    sequence: u16,
    profile: InfraredSimplexProfile,
    output: &mut [u8],
) -> Result<usize, InfraredFrameError> {
    let profile = profile.validate().map_err(profile_frame_error)?;
    if payload.is_empty() {
        return Err(InfraredFrameError::EmptyPayload);
    }
    if payload.len() > usize::from(profile.maximum_payload_bytes) {
        return Err(InfraredFrameError::OversizedPayload);
    }
    let length = HEADER_BYTES + payload.len() + INTEGRITY_BYTES;
    if output.len() < length {
        return Err(InfraredFrameError::OutputTooSmall);
    }
    output[..4].copy_from_slice(&PREAMBLE);
    output[4] = profile.revision;
    output[5] = profile.profile;
    output[6..8].copy_from_slice(&sequence.to_be_bytes());
    output[8..10].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    output[HEADER_BYTES..HEADER_BYTES + payload.len()].copy_from_slice(payload);
    let integrity = crc16_ccitt(&output[4..HEADER_BYTES + payload.len()]);
    output[HEADER_BYTES + payload.len()..length].copy_from_slice(&integrity.to_be_bytes());
    Ok(length)
}

pub fn decode_infrared_frame(
    bytes: &[u8],
    profile: InfraredSimplexProfile,
) -> Result<InfraredFrame<'_>, InfraredFrameError> {
    let profile = profile.validate().map_err(profile_frame_error)?;
    if bytes.len() < HEADER_BYTES + INTEGRITY_BYTES {
        return Err(InfraredFrameError::IncompleteFrame);
    }
    if bytes[..4] != PREAMBLE {
        return Err(InfraredFrameError::InvalidPreamble);
    }
    if bytes[4] != profile.revision {
        return Err(InfraredFrameError::UnsupportedRevision);
    }
    if bytes[5] != profile.profile {
        return Err(InfraredFrameError::UnsupportedProfile);
    }
    let payload_length = usize::from(u16::from_be_bytes([bytes[8], bytes[9]]));
    if payload_length == 0 || payload_length > usize::from(profile.maximum_payload_bytes) {
        return Err(InfraredFrameError::OversizedPayload);
    }
    let expected_length = HEADER_BYTES + payload_length + INTEGRITY_BYTES;
    if bytes.len() < expected_length {
        return Err(InfraredFrameError::IncompleteFrame);
    }
    if bytes.len() != expected_length {
        return Err(InfraredFrameError::LengthMismatch);
    }
    let integrity_offset = HEADER_BYTES + payload_length;
    let expected_integrity =
        u16::from_be_bytes([bytes[integrity_offset], bytes[integrity_offset + 1]]);
    if crc16_ccitt(&bytes[4..integrity_offset]) != expected_integrity {
        return Err(InfraredFrameError::IntegrityMismatch);
    }
    Ok(InfraredFrame {
        sequence: u16::from_be_bytes([bytes[6], bytes[7]]),
        payload: &bytes[HEADER_BYTES..integrity_offset],
    })
}

/// Finite ordering state for one exact receive direction. A carrier gap with
/// retained bytes must be reported as `IncompleteFrame`; it never becomes an
/// implicit retry or acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfraredReceiveSequence {
    next: u16,
    admitted_any: bool,
}

impl InfraredReceiveSequence {
    pub const fn new() -> Self {
        Self {
            next: 0,
            admitted_any: false,
        }
    }

    pub fn admit<'a>(&mut self, frame: InfraredFrame<'a>) -> Result<&'a [u8], InfraredFrameError> {
        if frame.sequence != self.next {
            if self.admitted_any && frame.sequence == self.next.wrapping_sub(1) {
                return Err(InfraredFrameError::DuplicateFrame);
            }
            return Err(InfraredFrameError::ReorderedFrame);
        }
        self.next = self.next.wrapping_add(1);
        self.admitted_any = true;
        Ok(frame.payload)
    }

    pub const fn admit_gap(retained_frame_bytes: usize) -> Result<(), InfraredFrameError> {
        if retained_frame_bytes == 0 {
            Ok(())
        } else {
            Err(InfraredFrameError::IncompleteFrame)
        }
    }
}

impl Default for InfraredReceiveSequence {
    fn default() -> Self {
        Self::new()
    }
}

fn profile_frame_error(error: InfraredProfileError) -> InfraredFrameError {
    match error {
        InfraredProfileError::UnsupportedRevision => InfraredFrameError::UnsupportedRevision,
        InfraredProfileError::UnsupportedProfile => InfraredFrameError::UnsupportedProfile,
        InfraredProfileError::InvalidPayloadLimit
        | InfraredProfileError::InvalidQueueLimit
        | InfraredProfileError::InvalidCarrier
        | InfraredProfileError::InvalidTiming => InfraredFrameError::UnsupportedProfile,
    }
}

fn crc16_ccitt(bytes: &[u8]) -> u16 {
    let mut crc = 0xffff_u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
