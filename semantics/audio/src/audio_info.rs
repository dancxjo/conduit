//! Canonical bounded PCM frame information.

use crate::SoundInfoError;
use alloc::vec::Vec;
use conduit_core::semantic_digest;
use serde::{Deserialize, Serialize};

pub const AUDIO_PCM_INFO_ID: &str = "audio/pcm-frames@1";
pub const PCM_FRAME_HEADER_ENCODED_LEN: usize = 29;
pub const MAXIMUM_PCM_FRAME_BYTES: u32 = 65_536;
pub const MAXIMUM_PCM_FRAMES_PER_BLOCK: u16 = 2_048;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PcmSampleRepresentation {
    Signed16LittleEndian,
    Signed24LittleEndian,
    Float32LittleEndian,
}

impl PcmSampleRepresentation {
    pub const fn bytes_per_sample(self) -> u8 {
        match self {
            Self::Signed16LittleEndian => 2,
            Self::Signed24LittleEndian => 3,
            Self::Float32LittleEndian => 4,
        }
    }
    const fn tag(self) -> u8 {
        match self {
            Self::Signed16LittleEndian => 0,
            Self::Signed24LittleEndian => 1,
            Self::Float32LittleEndian => 2,
        }
    }
    fn decode(actual: u8) -> Result<Self, SoundInfoError> {
        match actual {
            0 => Ok(Self::Signed16LittleEndian),
            1 => Ok(Self::Signed24LittleEndian),
            2 => Ok(Self::Float32LittleEndian),
            actual => Err(SoundInfoError::InvalidTag {
                field: "sample-representation",
                actual,
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PcmChannelLayout {
    Mono,
    StereoLeftRight,
}

impl PcmChannelLayout {
    pub const fn channels(self) -> u8 {
        match self {
            Self::Mono => 1,
            Self::StereoLeftRight => 2,
        }
    }
    const fn tag(self) -> u8 {
        match self {
            Self::Mono => 0,
            Self::StereoLeftRight => 1,
        }
    }
    fn decode(actual: u8) -> Result<Self, SoundInfoError> {
        match actual {
            0 => Ok(Self::Mono),
            1 => Ok(Self::StereoLeftRight),
            actual => Err(SoundInfoError::InvalidTag {
                field: "channel-layout",
                actual,
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PcmFrameHeader {
    pub representation: PcmSampleRepresentation,
    pub sample_rate_hz: u32,
    pub layout: PcmChannelLayout,
    pub frame_count: u16,
    pub clock_id: u64,
    pub start_frame: u64,
    pub discontinuity: bool,
    pub payload_bytes: u32,
}

impl PcmFrameHeader {
    pub fn new(
        representation: PcmSampleRepresentation,
        sample_rate_hz: u32,
        layout: PcmChannelLayout,
        frame_count: u16,
        clock_id: u64,
        start_frame: u64,
        discontinuity: bool,
    ) -> Result<Self, SoundInfoError> {
        if !(8_000..=192_000).contains(&sample_rate_hz) {
            return Err(SoundInfoError::OutOfRange("sample-rate-hz"));
        }
        if frame_count == 0 || frame_count > MAXIMUM_PCM_FRAMES_PER_BLOCK {
            return Err(SoundInfoError::OutOfRange("frame-count"));
        }
        if clock_id == 0 {
            return Err(SoundInfoError::OutOfRange("clock-id"));
        }
        let payload_bytes = u32::from(frame_count)
            * u32::from(layout.channels())
            * u32::from(representation.bytes_per_sample());
        if payload_bytes > MAXIMUM_PCM_FRAME_BYTES {
            return Err(SoundInfoError::OutOfRange("payload-bytes"));
        }
        Ok(Self {
            representation,
            sample_rate_hz,
            layout,
            frame_count,
            clock_id,
            start_frame,
            discontinuity,
            payload_bytes,
        })
    }

    pub fn validate_payload(self, payload: &[u8]) -> Result<(), SoundInfoError> {
        let actual = payload.len() as u32;
        if actual != self.payload_bytes {
            return Err(SoundInfoError::InconsistentPcmLength {
                expected: self.payload_bytes,
                actual,
            });
        }
        Ok(())
    }

    pub fn encode(self) -> [u8; PCM_FRAME_HEADER_ENCODED_LEN] {
        let mut out = [0; PCM_FRAME_HEADER_ENCODED_LEN];
        out[0] = self.representation.tag();
        out[1..5].copy_from_slice(&self.sample_rate_hz.to_le_bytes());
        out[5] = self.layout.tag();
        out[6..8].copy_from_slice(&self.frame_count.to_le_bytes());
        out[8..16].copy_from_slice(&self.clock_id.to_le_bytes());
        out[16..24].copy_from_slice(&self.start_frame.to_le_bytes());
        out[24] = u8::from(self.discontinuity);
        out[25..29].copy_from_slice(&self.payload_bytes.to_le_bytes());
        out
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        exact_length(encoded, PCM_FRAME_HEADER_ENCODED_LEN)?;
        let discontinuity = match encoded[24] {
            0 => false,
            1 => true,
            actual => {
                return Err(SoundInfoError::InvalidTag {
                    field: "discontinuity",
                    actual,
                })
            }
        };
        let header = Self::new(
            PcmSampleRepresentation::decode(encoded[0])?,
            u32::from_le_bytes(array(encoded, 1)?),
            PcmChannelLayout::decode(encoded[5])?,
            u16::from_le_bytes(array(encoded, 6)?),
            u64::from_le_bytes(array(encoded, 8)?),
            u64::from_le_bytes(array(encoded, 16)?),
            discontinuity,
        )?;
        let declared = u32::from_le_bytes(array(encoded, 25)?);
        if declared != header.payload_bytes {
            return Err(SoundInfoError::InconsistentPcmLength {
                expected: header.payload_bytes,
                actual: declared,
            });
        }
        Ok(header)
    }

    pub fn encode_frame(self, payload: &[u8]) -> Result<Vec<u8>, SoundInfoError> {
        self.validate_payload(payload)?;
        let mut encoded = Vec::with_capacity(PCM_FRAME_HEADER_ENCODED_LEN + payload.len());
        encoded.extend_from_slice(&self.encode());
        encoded.extend_from_slice(payload);
        Ok(encoded)
    }

    pub fn semantic_digest(self, payload: &[u8]) -> Result<[u8; 32], SoundInfoError> {
        Ok(semantic_digest(
            AUDIO_PCM_INFO_ID,
            &self.encode_frame(payload)?,
        ))
    }

    pub fn decode_frame(encoded: &[u8]) -> Result<(Self, &[u8]), SoundInfoError> {
        if encoded.len() < PCM_FRAME_HEADER_ENCODED_LEN {
            return Err(SoundInfoError::WrongLength {
                expected: PCM_FRAME_HEADER_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        let header = Self::decode(&encoded[..PCM_FRAME_HEADER_ENCODED_LEN])?;
        let payload = &encoded[PCM_FRAME_HEADER_ENCODED_LEN..];
        header.validate_payload(payload)?;
        Ok((header, payload))
    }
}

fn exact_length(encoded: &[u8], expected: usize) -> Result<(), SoundInfoError> {
    if encoded.len() != expected {
        return Err(SoundInfoError::WrongLength {
            expected,
            actual: encoded.len(),
        });
    }
    Ok(())
}

fn array<const N: usize>(encoded: &[u8], start: usize) -> Result<[u8; N], SoundInfoError> {
    encoded
        .get(start..start + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(SoundInfoError::WrongLength {
            expected: start + N,
            actual: encoded.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_frame_is_exact_bounded_and_canonical() {
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            48_000,
            PcmChannelLayout::StereoLeftRight,
            256,
            7,
            0,
            false,
        )
        .unwrap();
        assert_eq!(header.payload_bytes, 1_024);
        let frame = header.encode_frame(&[0; 1_024]).unwrap();
        let (decoded, payload) = PcmFrameHeader::decode_frame(&frame).unwrap();
        assert_eq!(decoded, header);
        assert_eq!(payload.len(), 1_024);
        assert_ne!(header.semantic_digest(payload).unwrap(), [0; 32]);
        assert!(matches!(
            header.validate_payload(&[0; 10]),
            Err(SoundInfoError::InconsistentPcmLength { .. })
        ));
    }
}
