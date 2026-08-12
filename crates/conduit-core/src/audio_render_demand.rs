//! Portable exact demand for one finite PCM render interval.

use crate::{semantic_digest, SoundInfoError};

pub const AUDIO_RENDER_DEMAND_INFO_ID: &str = "audio/render-demand@1";
pub const AUDIO_RENDER_DEMAND_ENCODED_LEN: usize = 22;

/// One exact finite interval requested on a named PCM sample clock.
///
/// This is media timing rather than a host callback or device mechanism. A
/// realization may obtain the demand from a timer, callback, or physical
/// device clock, but the portable synth sees only the exact interval it must
/// render.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AudioRenderDemand {
    pub clock_id: u64,
    pub start_frame: u64,
    pub frame_count: u16,
    pub sequence: u32,
}

impl AudioRenderDemand {
    pub fn new(
        clock_id: u64,
        start_frame: u64,
        frame_count: u16,
        sequence: u32,
    ) -> Result<Self, SoundInfoError> {
        if clock_id == 0 {
            return Err(SoundInfoError::OutOfRange("render-clock-id"));
        }
        if frame_count == 0 {
            return Err(SoundInfoError::OutOfRange("render-frame-count"));
        }
        start_frame
            .checked_add(u64::from(frame_count))
            .ok_or(SoundInfoError::OutOfRange("render-frame-interval"))?;
        Ok(Self {
            clock_id,
            start_frame,
            frame_count,
            sequence,
        })
    }

    pub fn encode(self) -> [u8; AUDIO_RENDER_DEMAND_ENCODED_LEN] {
        let mut out = [0; AUDIO_RENDER_DEMAND_ENCODED_LEN];
        out[0..8].copy_from_slice(&self.clock_id.to_le_bytes());
        out[8..16].copy_from_slice(&self.start_frame.to_le_bytes());
        out[16..18].copy_from_slice(&self.frame_count.to_le_bytes());
        out[18..22].copy_from_slice(&self.sequence.to_le_bytes());
        out
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        if encoded.len() != AUDIO_RENDER_DEMAND_ENCODED_LEN {
            return Err(SoundInfoError::WrongLength {
                expected: AUDIO_RENDER_DEMAND_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        Self::new(
            u64::from_le_bytes(array(encoded, 0)?),
            u64::from_le_bytes(array(encoded, 8)?),
            u16::from_le_bytes(array(encoded, 16)?),
            u32::from_le_bytes(array(encoded, 18)?),
        )
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(AUDIO_RENDER_DEMAND_INFO_ID, &self.encode())
    }
}

fn array<const N: usize>(encoded: &[u8], start: usize) -> Result<[u8; N], SoundInfoError> {
    encoded
        .get(start..start + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(SoundInfoError::WrongLength {
            expected: AUDIO_RENDER_DEMAND_ENCODED_LEN,
            actual: encoded.len(),
        })
}
