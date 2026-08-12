use super::{
    HostedPlaybackSelection, PlaybackFailure, PlaybackLifecycle, PlaybackMetrics, PlaybackReport,
    PERIOD_FRAMES, SAMPLE_RATE_HZ, SOURCE_CLOCK_ID,
};
use conduit_core::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FakePlaybackBehavior {
    Success,
    DeviceBusy,
    OpenFailure,
    UnderrunOnFirstBlock,
    ProviderLossOnFirstBlock,
    ProviderLossAfterFirstBlock,
    ProviderLossOnDrain,
    DrainFailure,
    CloseFailure,
}

pub(crate) struct FakePlaybackSession {
    selection: HostedPlaybackSelection,
    behavior: FakePlaybackBehavior,
    lifecycle: PlaybackLifecycle,
    metrics: PlaybackMetrics,
    expected_start_frame: Option<u64>,
}

impl FakePlaybackSession {
    pub(crate) fn new(selection: HostedPlaybackSelection, behavior: FakePlaybackBehavior) -> Self {
        Self {
            selection,
            behavior,
            lifecycle: PlaybackLifecycle::ResolvedAvailable,
            metrics: PlaybackMetrics {
                blocks_committed: 0,
                frames_committed: 0,
                discontinuities: 0,
                underruns: 0,
                first_commit_micros: None,
                minimum_write_micros: None,
                maximum_write_micros: None,
                total_write_micros: 0,
                period_frames: super::PERIOD_FRAMES,
                buffer_frames: super::BUFFER_FRAMES,
            },
            expected_start_frame: None,
        }
    }

    pub(crate) fn write_frame(&mut self, encoded: &[u8]) -> Result<(), PlaybackFailure> {
        let (header, _) =
            PcmFrameHeader::decode_frame(encoded).map_err(|_| PlaybackFailure::InvalidPcm)?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.sample_rate_hz != SAMPLE_RATE_HZ
            || header.layout != PcmChannelLayout::StereoLeftRight
            || header.frame_count > PERIOD_FRAMES
            || header.clock_id != SOURCE_CLOCK_ID
        {
            return Err(PlaybackFailure::InvalidPcm);
        }
        if self
            .expected_start_frame
            .is_some_and(|expected| expected != header.start_frame && !header.discontinuity)
        {
            return Err(PlaybackFailure::DiscontinuousInput);
        }
        if self.metrics.blocks_committed == 0 {
            self.lifecycle = PlaybackLifecycle::OpenedPrepared;
            if self.behavior == FakePlaybackBehavior::DeviceBusy {
                self.lifecycle = PlaybackLifecycle::Failed;
                return Err(PlaybackFailure::DeviceBusy);
            }
            if self.behavior == FakePlaybackBehavior::OpenFailure {
                self.lifecycle = PlaybackLifecycle::Failed;
                return Err(PlaybackFailure::OpenFailed);
            }
            self.lifecycle = PlaybackLifecycle::Started;
            match self.behavior {
                FakePlaybackBehavior::UnderrunOnFirstBlock => {
                    self.metrics.underruns = 1;
                    self.lifecycle = PlaybackLifecycle::Underrun;
                    return Err(PlaybackFailure::Underrun);
                }
                FakePlaybackBehavior::ProviderLossOnFirstBlock => {
                    self.lifecycle = PlaybackLifecycle::ProviderLost;
                    return Err(PlaybackFailure::ProviderLost);
                }
                _ => {}
            }
        } else if self.behavior == FakePlaybackBehavior::ProviderLossAfterFirstBlock {
            self.lifecycle = PlaybackLifecycle::ProviderLost;
            return Err(PlaybackFailure::ProviderLost);
        }
        self.metrics.blocks_committed += 1;
        self.metrics.frames_committed += u64::from(header.frame_count);
        self.expected_start_frame = Some(header.start_frame + u64::from(header.frame_count));
        self.lifecycle = if self.metrics.blocks_committed == 1 {
            PlaybackLifecycle::FirstFrameCommitted
        } else {
            PlaybackLifecycle::Active
        };
        Ok(())
    }

    pub(crate) fn drain(&mut self) -> Result<(), PlaybackFailure> {
        self.lifecycle = PlaybackLifecycle::DrainRequested;
        if self.behavior == FakePlaybackBehavior::ProviderLossOnDrain {
            self.lifecycle = PlaybackLifecycle::ProviderLost;
            return Err(PlaybackFailure::ProviderLost);
        }
        if self.behavior == FakePlaybackBehavior::DrainFailure {
            self.lifecycle = PlaybackLifecycle::Failed;
            return Err(PlaybackFailure::DrainFailed);
        }
        self.lifecycle = PlaybackLifecycle::Drained;
        self.lifecycle = PlaybackLifecycle::StoppedClosed;
        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<(), PlaybackFailure> {
        if self.behavior == FakePlaybackBehavior::CloseFailure {
            self.lifecycle = PlaybackLifecycle::Failed;
            return Err(PlaybackFailure::CloseFailed);
        }
        self.lifecycle = PlaybackLifecycle::StoppedClosed;
        Ok(())
    }

    pub(crate) const fn lifecycle(&self) -> PlaybackLifecycle {
        self.lifecycle
    }

    pub(crate) fn report(&self) -> PlaybackReport {
        PlaybackReport {
            backend: "deterministic-playback-fixture@1",
            resource_pool_id: self.selection.pool_id().as_str().to_owned(),
            alsa_target: self.selection.alsa_target(),
            lifecycle: self.lifecycle,
            metrics: self.metrics,
            timing_class: "deterministic-fixture-not-live-device",
            clock_correlation: "fixture-exact",
            controlled_staging_bytes: 0,
            external_buffer_class: "fixture-none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_audio::AlsaPlaybackObservation;
    use conduit_core::{BootId, OfferGeneration};

    fn selection() -> HostedPlaybackSelection {
        HostedPlaybackSelection::deterministic_fake(
            AlsaPlaybackObservation {
                card_index: 0,
                card_id: "FIXTURE".into(),
                card_name: "fixture".into(),
                device: 0,
                device_name: "fixture".into(),
                base_identity: "fixture-base".into(),
            },
            BootId::from("fixture-boot"),
            OfferGeneration(1),
            FakePlaybackBehavior::Success,
        )
    }

    fn frame(
        representation: PcmSampleRepresentation,
        sample_rate_hz: u32,
        layout: PcmChannelLayout,
    ) -> Vec<u8> {
        let channels = match layout {
            PcmChannelLayout::Mono => 1,
            PcmChannelLayout::StereoLeftRight => 2,
        };
        let bytes_per_sample = match representation {
            PcmSampleRepresentation::Signed16LittleEndian => 2,
            PcmSampleRepresentation::Signed24LittleEndian => 3,
            PcmSampleRepresentation::Float32LittleEndian => 4,
        };
        PcmFrameHeader::new(
            representation,
            sample_rate_hz,
            layout,
            PERIOD_FRAMES,
            SOURCE_CLOCK_ID,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&vec![
            0;
            PERIOD_FRAMES as usize * channels * bytes_per_sample
        ])
        .unwrap()
    }

    #[test]
    fn unsupported_representation_rate_and_layout_refuse_before_open() {
        for encoded in [
            frame(
                PcmSampleRepresentation::Float32LittleEndian,
                SAMPLE_RATE_HZ,
                PcmChannelLayout::StereoLeftRight,
            ),
            frame(
                PcmSampleRepresentation::Signed16LittleEndian,
                44_100,
                PcmChannelLayout::StereoLeftRight,
            ),
            frame(
                PcmSampleRepresentation::Signed16LittleEndian,
                SAMPLE_RATE_HZ,
                PcmChannelLayout::Mono,
            ),
        ] {
            let mut session = FakePlaybackSession::new(selection(), FakePlaybackBehavior::Success);
            assert_eq!(
                session.write_frame(&encoded),
                Err(PlaybackFailure::InvalidPcm)
            );
            assert_eq!(session.lifecycle(), PlaybackLifecycle::ResolvedAvailable);
            assert_eq!(session.metrics.blocks_committed, 0);
        }
    }

    #[test]
    fn provider_loss_after_first_block_is_active_pcm_loss() {
        let encoded = frame(
            PcmSampleRepresentation::Signed16LittleEndian,
            SAMPLE_RATE_HZ,
            PcmChannelLayout::StereoLeftRight,
        );
        let mut session = FakePlaybackSession::new(
            selection(),
            FakePlaybackBehavior::ProviderLossAfterFirstBlock,
        );
        session.write_frame(&encoded).unwrap();
        assert_eq!(session.lifecycle(), PlaybackLifecycle::FirstFrameCommitted);
        assert_eq!(session.metrics.blocks_committed, 1);

        let mut second = encoded;
        second[16..24].copy_from_slice(&u64::from(PERIOD_FRAMES).to_le_bytes());
        assert_eq!(
            session.write_frame(&second),
            Err(PlaybackFailure::ProviderLost)
        );
        assert_eq!(session.lifecycle(), PlaybackLifecycle::ProviderLost);
        assert_eq!(session.metrics.blocks_committed, 1);
    }

    #[test]
    fn cancellation_closes_exactly_from_each_nonterminal_lifecycle() {
        for lifecycle in [
            PlaybackLifecycle::ResolvedAvailable,
            PlaybackLifecycle::OpenedPrepared,
            PlaybackLifecycle::Started,
            PlaybackLifecycle::FirstFrameCommitted,
            PlaybackLifecycle::Active,
            PlaybackLifecycle::DrainRequested,
        ] {
            let mut session = FakePlaybackSession::new(selection(), FakePlaybackBehavior::Success);
            session.lifecycle = lifecycle;
            session.stop().unwrap();
            assert_eq!(session.lifecycle(), PlaybackLifecycle::StoppedClosed);
            session.stop().unwrap();
            assert_eq!(session.lifecycle(), PlaybackLifecycle::StoppedClosed);
        }
    }

    #[test]
    fn injected_cleanup_failure_is_not_clean_completion() {
        let mut session = FakePlaybackSession::new(selection(), FakePlaybackBehavior::CloseFailure);
        session.lifecycle = PlaybackLifecycle::Active;
        assert_eq!(session.stop(), Err(PlaybackFailure::CloseFailed));
        assert_eq!(session.lifecycle(), PlaybackLifecycle::Failed);
    }
}
