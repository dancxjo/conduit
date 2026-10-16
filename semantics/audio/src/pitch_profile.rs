//! One finite window per immutable PCM block; no implicit streaming history.
use crate::{
    PcmFrameHeader, PcmPitchObservation, PcmPitchRefusal, PcmPitchWorkspace, PcmTimeAnalysisRefusal,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmPitchProfile {
    clock_id: u64,
    sample_rate_hz: u32,
    source_frames: u16,
    offset: u16,
    frames: u16,
    minimum_lag: u16,
    maximum_lag: u16,
}
impl PcmPitchProfile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        clock_id: u64,
        sample_rate_hz: u32,
        source_frames: u16,
        offset: u16,
        frames: u16,
        minimum_lag: u16,
        maximum_lag: u16,
    ) -> Result<Self, PcmPitchRefusal> {
        if clock_id == 0 || !(8000..=192000).contains(&sample_rate_hz) {
            return Err(PcmPitchRefusal::Input(
                PcmTimeAnalysisRefusal::WrongTimebase,
            ));
        }
        if !(64..=2048).contains(&source_frames)
            || !(64..=2048).contains(&frames)
            || u32::from(offset) + u32::from(frames) > u32::from(source_frames)
        {
            return Err(PcmPitchRefusal::UnsupportedWindow);
        }
        if minimum_lag < 2
            || minimum_lag > maximum_lag
            || maximum_lag > 512
            || u32::from(maximum_lag) * 3 > u32::from(frames)
        {
            return Err(PcmPitchRefusal::InvalidLagRange);
        }
        Ok(Self {
            clock_id,
            sample_rate_hz,
            source_frames,
            offset,
            frames,
            minimum_lag,
            maximum_lag,
        })
    }
    pub fn clock_id(self) -> u64 {
        self.clock_id
    }
    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }
    pub fn source_frames(self) -> u16 {
        self.source_frames
    }
    pub fn observe(
        self,
        workspace: &mut PcmPitchWorkspace,
        encoded: &[u8],
    ) -> Result<PcmPitchObservation, PcmPitchRefusal> {
        let (header, _) = PcmFrameHeader::decode_frame(encoded)
            .map_err(|e| PcmPitchRefusal::Input(PcmTimeAnalysisRefusal::Frame(e)))?;
        if header.frame_count() != self.source_frames {
            return Err(PcmPitchRefusal::WrongSourceFrames);
        }
        workspace.observe(
            encoded,
            self.clock_id,
            self.sample_rate_hz,
            self.offset,
            self.frames,
            self.minimum_lag,
            self.maximum_lag,
        )
    }
}
