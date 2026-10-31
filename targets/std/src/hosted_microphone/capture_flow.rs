//! Prepared finite PCM block storage. Acquisition time is capture-relative,
//! never wall time, and a clock nonce is reserved before starting a provider.
use super::*;
use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, PCM_FRAME_HEADER_ENCODED_LEN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrophoneFlowLimits {
    pub total_frames: u32,
    pub block_frames: u16,
}
impl MicrophoneFlowLimits {
    pub fn block_count(self) -> Result<u16, MicrophoneFailure> {
        if self.total_frames == 0
            || self.total_frames > conduit_audio::MAXIMUM_PCM_CLIP_FRAMES
            || self.block_frames == 0
            || self.block_frames > conduit_audio::MAXIMUM_PCM_FRAMES_PER_BLOCK
        {
            return Err(MicrophoneFailure::InvalidLimits);
        }
        let count = self.total_frames.div_ceil(u32::from(self.block_frames));
        if count as usize > conduit_audio::MAXIMUM_PCM_CLIP_BLOCKS {
            return Err(MicrophoneFailure::InvalidLimits);
        }
        Ok(count as u16)
    }
}

/// An opaque capture-relative sample clock, not an acquisition timestamp.
/// Entropy failure refuses preparation; there is no fallback to constant1.
#[derive(Debug, PartialEq, Eq)]
pub struct CaptureSampleClock(u64);
impl CaptureSampleClock {
    pub fn reserve() -> Result<Self, MicrophoneFailure> {
        let mut bytes = [0; 8];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(|_| MicrophoneFailure::ClockUnavailable)?;
        let value = u64::from_le_bytes(bytes);
        if value == 0 {
            return Err(MicrophoneFailure::ClockUnavailable);
        }
        Ok(Self(value))
    }
    pub const fn id(&self) -> u64 {
        self.0
    }
}

/// One allocation at preparation, reused for every block including a short tail.
/// Callers may expose a frame only after the block is complete. The finite
/// provider loop must continue through EOF/status to establish completion.
pub struct PreparedPcmCapture {
    limits: MicrophoneFlowLimits,
    clock: CaptureSampleClock,
    storage: Vec<u8>,
    filled: usize,
    start_frame: u64,
    ready: bool,
}
impl PreparedPcmCapture {
    pub fn prepare(
        limits: MicrophoneFlowLimits,
        clock: CaptureSampleClock,
    ) -> Result<Self, MicrophoneFailure> {
        limits.block_count()?;
        Ok(Self {
            limits,
            clock,
            storage: vec![0; PCM_FRAME_HEADER_ENCODED_LEN + usize::from(limits.block_frames) * 2],
            filled: 0,
            start_frame: 0,
            ready: false,
        })
    }
    pub fn limits(&self) -> MicrophoneFlowLimits {
        self.limits
    }
    pub fn clock(&self) -> &CaptureSampleClock {
        &self.clock
    }
    fn block_frames(&self) -> u16 {
        (u64::from(self.limits.total_frames) - self.start_frame)
            .min(u64::from(self.limits.block_frames)) as u16
    }
    pub fn remaining_block_bytes(&self) -> usize {
        usize::from(self.block_frames()) * 2 - self.filled
    }
    pub fn raw_frames_received(&self) -> u64 {
        self.start_frame + (self.filled / 2) as u64
    }
    pub fn accept(&mut self, bytes: &[u8]) -> Result<(), MicrophoneFailure> {
        if self.ready {
            return Err(MicrophoneFailure::CaptureOverrun);
        }
        if bytes.len() > self.remaining_block_bytes() {
            return Err(MicrophoneFailure::OutputOverflow);
        }
        let begin = PCM_FRAME_HEADER_ENCODED_LEN + self.filled;
        self.storage[begin..begin + bytes.len()].copy_from_slice(bytes);
        self.filled += bytes.len();
        if self.filled != 0 && self.remaining_block_bytes() == 0 {
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                SAMPLE_RATE_HZ,
                PcmChannelLayout::Mono,
                self.block_frames(),
                self.clock.id(),
                self.start_frame,
                false,
            )
            .map_err(|_| MicrophoneFailure::ReadFailed)?;
            self.storage[..PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
            self.ready = true;
        }
        Ok(())
    }
    pub fn frame(&self) -> Option<&[u8]> {
        self.ready
            .then_some(&self.storage[..PCM_FRAME_HEADER_ENCODED_LEN + self.filled])
    }
    pub fn release_frame(&mut self) -> Result<(), MicrophoneFailure> {
        if !self.ready {
            return Err(MicrophoneFailure::ReadFailed);
        }
        self.start_frame += u64::from(self.block_frames());
        self.filled = 0;
        self.ready = false;
        Ok(())
    }
    pub fn finish(&self) -> Result<(), MicrophoneFailure> {
        if !self.filled.is_multiple_of(2) {
            return Err(MicrophoneFailure::PartialSampleFrame);
        }
        if self.raw_frames_received() != u64::from(self.limits.total_frames) {
            return Err(MicrophoneFailure::ShortCapture);
        }
        Ok(())
    }
    pub fn storage_capacity(&self) -> usize {
        self.storage.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_bytes_short_tail_clock_and_offsets_survive_reuse() {
        let limits = MicrophoneFlowLimits {
            total_frames: 5,
            block_frames: 3,
        };
        let mut capture = PreparedPcmCapture::prepare(limits, CaptureSampleClock(91)).unwrap();
        let capacity = capture.storage_capacity();
        let address = capture.storage.as_ptr();
        let probe = crate::allocation_probe::begin();
        capture.accept(&[1, 2, 3]).unwrap();
        assert!(capture.frame().is_none());
        capture.accept(&[4, 5, 6]).unwrap();
        let (header, payload) = PcmFrameHeader::decode_frame(capture.frame().unwrap()).unwrap();
        assert_eq!(
            (
                header.clock_id(),
                header.start_frame(),
                header.frame_count()
            ),
            (91, 0, 3)
        );
        assert_eq!(payload, [1, 2, 3, 4, 5, 6]);
        capture.release_frame().unwrap();
        capture.accept(&[7, 8, 9, 10]).unwrap();
        let (header, payload) = PcmFrameHeader::decode_frame(capture.frame().unwrap()).unwrap();
        assert_eq!(
            (
                header.clock_id(),
                header.start_frame(),
                header.frame_count()
            ),
            (91, 3, 2)
        );
        assert_eq!(payload, [7, 8, 9, 10]);
        capture.finish().unwrap();
        capture.release_frame().unwrap();
        capture.finish().unwrap();
        assert_eq!(
            probe.finish(),
            0,
            "block packing and decoding allocate nothing"
        );
        assert_eq!(capture.storage_capacity(), capacity);
        assert_eq!(capture.storage.as_ptr(), address);
        assert_eq!(
            capture.accept(&[0, 0]),
            Err(MicrophoneFailure::OutputOverflow)
        );
    }
    #[test]
    fn bounds_partial_samples_short_input_and_pressure_are_distinct() {
        for limits in [
            MicrophoneFlowLimits {
                total_frames: 0,
                block_frames: 1,
            },
            MicrophoneFlowLimits {
                total_frames: 96001,
                block_frames: 2048,
            },
            MicrophoneFlowLimits {
                total_frames: 65,
                block_frames: 1,
            },
        ] {
            assert!(limits.block_count().is_err());
        }
        let mut capture = PreparedPcmCapture::prepare(
            MicrophoneFlowLimits {
                total_frames: 2,
                block_frames: 2,
            },
            CaptureSampleClock(8),
        )
        .unwrap();
        capture.accept(&[0]).unwrap();
        assert_eq!(capture.finish(), Err(MicrophoneFailure::PartialSampleFrame));
        capture.accept(&[1]).unwrap();
        assert_eq!(capture.finish(), Err(MicrophoneFailure::ShortCapture));
        capture.accept(&[2, 3]).unwrap();
        assert_eq!(capture.accept(&[]), Err(MicrophoneFailure::CaptureOverrun));
    }
}
