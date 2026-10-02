//! Explicit aggregate work limits; instantaneous queues remain unchanged.
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{Failure, FailureCode};
#[derive(Clone, Copy)]
pub(super) struct AudioStreamBudget {
    pub blocks: u32,
    pub millis: u32,
    sample_rate_hz: u32,
    frames: u64,
}
impl AudioStreamBudget {
    pub fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let value = |key: &str, max: u32| {
            let mut entries = placement.configuration.iter().filter(|v| v.key == key);
            let entry = entries
                .next()
                .ok_or_else(|| format!("missing audio budget {key}"))?;
            if entries.next().is_some() {
                return Err("duplicate audio budget".into());
            }
            match entry.value {
                ConfigurationValue::U64(v) if v > 0 && v <= u64::from(max) => Ok(v as u32),
                _ => Err(format!("invalid audio budget {key}")),
            }
        };
        // These are the already admitted profiles of the installed consumers;
        // this counter does not infer duration from the source clock position.
        let sample_rate_hz = match placement.implementation_id.as_str() {
            conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION => 22_050,
            conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
            | conduit_std_offers::AUDIO_WAV_ARTIFACT_IMPLEMENTATION
            | conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION => 48_000,
            _ => return Err("unknown installed audio work profile".into()),
        };
        Ok(Self {
            sample_rate_hz,
            frames: 0,
            blocks: value(
                conduit_semantic_catalog::AUDIO_MAXIMUM_BLOCKS_KEY,
                conduit_semantic_catalog::AUDIO_STREAM_MAXIMUM_BLOCKS,
            )?,
            millis: value(
                conduit_semantic_catalog::AUDIO_MAXIMUM_MILLIS_KEY,
                conduit_semantic_catalog::AUDIO_STREAM_MAXIMUM_MILLIS,
            )?,
        })
    }
    #[cfg(test)]
    pub fn playback_for_test() -> Self {
        Self {
            blocks: 3072,
            millis: 16384,
            sample_rate_hz: 48000,
            frames: 0,
        }
    }

    pub fn frame(&mut self, encoded: &[u8]) -> Result<(), Failure> {
        let invalid = Failure {
            code: FailureCode::WorkBudgetExhausted,
            detail: 185,
        };
        let (header, _) =
            conduit_audio::PcmFrameHeader::decode_frame(encoded).map_err(|_| invalid)?;
        if header.sample_rate_hz != self.sample_rate_hz {
            return Err(Failure {
                code: FailureCode::InvalidInput,
                detail: 185,
            });
        }
        let frames = self
            .frames
            .checked_add(u64::from(header.frame_count))
            .ok_or(invalid)?;
        if frames.checked_mul(1000).ok_or(invalid)?
            > u64::from(self.sample_rate_hz) * u64::from(self.millis)
        {
            return Err(invalid);
        }
        self.frames = frames;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
    fn block(rate: u32, frames: u16, start: u64) -> Vec<u8> {
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            rate,
            PcmChannelLayout::Mono,
            frames,
            7,
            start,
            false,
        )
        .unwrap();
        let mut bytes = header.encode().to_vec();
        bytes.resize(bytes.len() + usize::from(frames) * 2, 0);
        bytes
    }
    #[test]
    fn duration_counts_consumed_frames_not_absolute_clock_position() {
        let mut budget = AudioStreamBudget {
            blocks: 32768,
            millis: 30000,
            sample_rate_hz: 48000,
            frames: 0,
        };
        for index in 0..6000 {
            budget
                .frame(&block(48000, 240, 9_000_000 + index * 240))
                .unwrap();
        }
        assert_eq!(budget.frames, 30 * 48000);
        let error = budget.frame(&block(48000, 1, 10_440_000)).unwrap_err();
        assert_eq!(error.code, FailureCode::WorkBudgetExhausted);
        assert_eq!(
            budget.frames,
            30 * 48000,
            "refusal must not consume further budget"
        );
    }
    #[test]
    fn unsupported_profile_is_refused_before_it_can_undercharge_duration() {
        let mut budget = AudioStreamBudget::playback_for_test();
        assert_eq!(
            budget.frame(&block(96000, 100, 0)).unwrap_err().code,
            FailureCode::InvalidInput
        );
        assert_eq!(budget.frames, 0);
    }
}
