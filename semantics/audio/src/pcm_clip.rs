//! Canonical finite PCM clip composed from exact contiguous PCM frames.

use crate::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, PCM_FRAME_HEADER_ENCODED_LEN,
};
use alloc::vec::Vec;

pub const AUDIO_PCM_CLIP_INFO_ID: &str = "audio/pcm-clip@1";
pub const MAXIMUM_PCM_CLIP_BLOCKS: usize = 64;
pub const MAXIMUM_PCM_CLIP_FRAMES: u32 = 96_000;
pub const MAXIMUM_PCM_CLIP_BYTES: usize = 786_432;

const MAGIC: &[u8; 8] = b"CDTPCM01";
const HEADER_BYTES: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmClipError {
    Empty,
    TooManyBlocks,
    BoundExceeded,
    MalformedFrame,
    MixedProfile,
    MixedClock,
    Discontinuity,
    NonContiguous,
    FrameCountOverflow,
    DeclaredExtentMismatch,
    TrailingBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmClipProfile {
    pub representation: PcmSampleRepresentation,
    pub sample_rate_hz: u32,
    pub layout: PcmChannelLayout,
    pub clock_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmClipFrame<'a> {
    pub header: PcmFrameHeader,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmClip<'a> {
    pub profile: PcmClipProfile,
    pub frame_count: u32,
    pub blocks: Vec<PcmClipFrame<'a>>,
}

pub fn encode_pcm_clip(frames: &[&[u8]]) -> Result<Vec<u8>, PcmClipError> {
    let decoded = validate_frames(frames)?;
    let frames_bytes = frames.iter().try_fold(0_usize, |total, frame| {
        total
            .checked_add(4)
            .and_then(|total| total.checked_add(frame.len()))
            .ok_or(PcmClipError::BoundExceeded)
    })?;
    let capacity = HEADER_BYTES
        .checked_add(frames_bytes)
        .ok_or(PcmClipError::BoundExceeded)?;
    if capacity > MAXIMUM_PCM_CLIP_BYTES {
        return Err(PcmClipError::BoundExceeded);
    }
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    encoded.extend_from_slice(&decoded.frame_count.to_le_bytes());
    for frame in frames {
        encoded.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        encoded.extend_from_slice(frame);
    }
    Ok(encoded)
}

pub fn decode_pcm_clip(encoded: &[u8]) -> Result<PcmClip<'_>, PcmClipError> {
    if encoded.len() > MAXIMUM_PCM_CLIP_BYTES {
        return Err(PcmClipError::BoundExceeded);
    }
    if encoded.len() < HEADER_BYTES || encoded.get(..8) != Some(MAGIC) {
        return Err(PcmClipError::MalformedFrame);
    }
    let block_count = u16::from_le_bytes([encoded[8], encoded[9]]) as usize;
    if block_count == 0 {
        return Err(PcmClipError::Empty);
    }
    if block_count > MAXIMUM_PCM_CLIP_BLOCKS {
        return Err(PcmClipError::TooManyBlocks);
    }
    let declared_frames = u32::from_le_bytes(encoded[10..14].try_into().unwrap());
    let mut offset = HEADER_BYTES;
    let mut raw = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        let length_end = offset.checked_add(4).ok_or(PcmClipError::BoundExceeded)?;
        let length = encoded
            .get(offset..length_end)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(PcmClipError::MalformedFrame)? as usize;
        if length < PCM_FRAME_HEADER_ENCODED_LEN {
            return Err(PcmClipError::MalformedFrame);
        }
        offset = length_end;
        let frame_end = offset
            .checked_add(length)
            .ok_or(PcmClipError::BoundExceeded)?;
        raw.push(
            encoded
                .get(offset..frame_end)
                .ok_or(PcmClipError::MalformedFrame)?,
        );
        offset = frame_end;
    }
    if offset != encoded.len() {
        return Err(PcmClipError::TrailingBytes);
    }
    let clip = validate_frames(&raw)?;
    if clip.frame_count != declared_frames {
        return Err(PcmClipError::DeclaredExtentMismatch);
    }
    Ok(clip)
}

fn validate_frames<'a>(frames: &[&'a [u8]]) -> Result<PcmClip<'a>, PcmClipError> {
    if frames.is_empty() {
        return Err(PcmClipError::Empty);
    }
    if frames.len() > MAXIMUM_PCM_CLIP_BLOCKS {
        return Err(PcmClipError::TooManyBlocks);
    }
    let mut blocks = Vec::with_capacity(frames.len());
    let mut profile: Option<PcmClipProfile> = None;
    let mut expected_start = None;
    let mut total_frames = 0_u32;
    for encoded in frames {
        let (header, payload) =
            PcmFrameHeader::decode_frame(encoded).map_err(|_| PcmClipError::MalformedFrame)?;
        if header.discontinuity {
            return Err(PcmClipError::Discontinuity);
        }
        let current = PcmClipProfile {
            representation: header.representation,
            sample_rate_hz: header.sample_rate_hz,
            layout: header.layout,
            clock_id: header.clock_id,
        };
        if let Some(first) = profile {
            if current.representation != first.representation
                || current.sample_rate_hz != first.sample_rate_hz
                || current.layout != first.layout
            {
                return Err(PcmClipError::MixedProfile);
            }
            if current.clock_id != first.clock_id {
                return Err(PcmClipError::MixedClock);
            }
        } else {
            profile = Some(current);
        }
        if expected_start.is_some_and(|expected| header.start_frame != expected) {
            return Err(PcmClipError::NonContiguous);
        }
        expected_start = Some(
            header
                .start_frame
                .checked_add(u64::from(header.frame_count))
                .ok_or(PcmClipError::FrameCountOverflow)?,
        );
        total_frames = total_frames
            .checked_add(u32::from(header.frame_count))
            .ok_or(PcmClipError::FrameCountOverflow)?;
        if total_frames > MAXIMUM_PCM_CLIP_FRAMES {
            return Err(PcmClipError::BoundExceeded);
        }
        blocks.push(PcmClipFrame { header, payload });
    }
    Ok(PcmClip {
        profile: profile.unwrap(),
        frame_count: total_frames,
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(start: u64, samples: &[i16]) -> Vec<u8> {
        let payload = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            PcmChannelLayout::Mono,
            samples.len() as u16,
            7,
            start,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap()
    }

    #[test]
    fn multiple_contiguous_frames_round_trip_canonically() {
        let first = frame(20, &[1, 2, 3]);
        let second = frame(23, &[4, 5]);
        let encoded = encode_pcm_clip(&[&first, &second]).unwrap();
        let decoded = decode_pcm_clip(&encoded).unwrap();
        assert_eq!(decoded.blocks.len(), 2);
        assert_eq!(decoded.frame_count, 5);
        assert_eq!(decoded.profile.sample_rate_hz, 16_000);
        assert_eq!(decoded.blocks[1].payload, &[4, 0, 5, 0]);
        assert_eq!(encode_pcm_clip(&[&first, &second]).unwrap(), encoded);
    }

    #[test]
    fn gaps_mixed_clocks_and_trailing_bytes_fail_distinctly() {
        let first = frame(0, &[1, 2]);
        let gap = frame(3, &[3]);
        assert_eq!(
            encode_pcm_clip(&[&first, &gap]),
            Err(PcmClipError::NonContiguous)
        );

        let payload = 3_i16.to_le_bytes();
        let wrong_clock = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            PcmChannelLayout::Mono,
            1,
            8,
            2,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap();
        assert_eq!(
            encode_pcm_clip(&[&first, &wrong_clock]),
            Err(PcmClipError::MixedClock)
        );

        let mut encoded = encode_pcm_clip(&[&first]).unwrap();
        encoded.push(0);
        assert_eq!(decode_pcm_clip(&encoded), Err(PcmClipError::TrailingBytes));
    }
}
