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
