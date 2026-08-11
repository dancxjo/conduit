use super::{
    discover_alsa_playback, HostedPlaybackSelection, BUFFER_FRAMES, PERIOD_FRAMES, SAMPLE_RATE_HZ,
    SOURCE_CLOCK_ID,
};
use conduit_core::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use std::io::{Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::time::Instant;

const MAXIMUM_DIAGNOSTIC_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackLifecycle {
    ResolvedAvailable,
    OpenedPrepared,
    Started,
    FirstFrameCommitted,
    Active,
    DrainRequested,
    Drained,
    StoppedClosed,
    Underrun,
    ProviderLost,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackFailure {
    StaleObservation,
    DeviceBusy,
    OpenFailed,
    InvalidPcm,
    DiscontinuousInput,
    Underrun,
    ProviderLost,
    WriteFailed,
    DrainFailed,
    CloseFailed,
    InvalidLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackMetrics {
    pub blocks_committed: u32,
    pub frames_committed: u64,
    pub discontinuities: u32,
    pub underruns: u32,
    pub first_commit_micros: Option<u64>,
    pub minimum_write_micros: Option<u64>,
    pub maximum_write_micros: Option<u64>,
    pub total_write_micros: u64,
    pub period_frames: u16,
    pub buffer_frames: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackReport {
    pub backend: &'static str,
    pub resource_pool_id: String,
    pub alsa_target: String,
    pub lifecycle: PlaybackLifecycle,
    pub metrics: PlaybackMetrics,
    pub timing_class: &'static str,
    pub clock_correlation: &'static str,
    pub controlled_staging_bytes: u32,
    pub external_buffer_class: &'static str,
}

impl PlaybackMetrics {
    const fn new() -> Self {
        Self {
            blocks_committed: 0,
            frames_committed: 0,
            discontinuities: 0,
            underruns: 0,
            first_commit_micros: None,
            minimum_write_micros: None,
            maximum_write_micros: None,
            total_write_micros: 0,
            period_frames: PERIOD_FRAMES,
            buffer_frames: BUFFER_FRAMES,
        }
    }
}

/// Finite direct-hardware playback session. The process is absent until the
/// first admitted PCM block crosses the host-operation boundary.
pub struct AlsaAplaySession {
    selection: HostedPlaybackSelection,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr: Option<ChildStderr>,
    lifecycle: PlaybackLifecycle,
    metrics: PlaybackMetrics,
    expected_start_frame: Option<u64>,
    play_started: Option<Instant>,
}

impl AlsaAplaySession {
    pub fn resolved(selection: HostedPlaybackSelection) -> Self {
        Self {
            selection,
            child: None,
            stdin: None,
            stderr: None,
            lifecycle: PlaybackLifecycle::ResolvedAvailable,
            metrics: PlaybackMetrics::new(),
            expected_start_frame: None,
            play_started: None,
        }
    }

    pub const fn lifecycle(&self) -> PlaybackLifecycle {
        self.lifecycle
    }

    pub const fn metrics(&self) -> PlaybackMetrics {
        self.metrics
    }

    pub fn report(&self) -> PlaybackReport {
        PlaybackReport {
            backend: "alsa-aplay-direct-hw@1",
            resource_pool_id: self.selection.pool_id().as_str().to_owned(),
            alsa_target: self.selection.alsa_target(),
            lifecycle: self.lifecycle,
            metrics: self.metrics,
            timing_class: "measured-hosted-best-effort",
            clock_correlation: "first-commit-monotonic-observed-no-hardware-timestamp-guarantee",
            controlled_staging_bytes: 0,
            external_buffer_class: "configured-alsa-hw-1024-frames",
        }
    }

    pub fn write_frame(&mut self, encoded: &[u8]) -> Result<(), PlaybackFailure> {
        let (header, payload) =
            PcmFrameHeader::decode_frame(encoded).map_err(|_| PlaybackFailure::InvalidPcm)?;
        self.validate_header(header)?;
        if self.child.is_none() {
            self.open()?;
        }
        if self.child_exited()? {
            return Err(self.classify_terminal_failure());
        }
        let started = Instant::now();
        self.stdin
            .as_mut()
            .ok_or(PlaybackFailure::InvalidLifecycle)?
            .write_all(payload)
            .map_err(|_| PlaybackFailure::WriteFailed)?;
        self.stdin
            .as_mut()
            .ok_or(PlaybackFailure::InvalidLifecycle)?
            .flush()
            .map_err(|_| PlaybackFailure::WriteFailed)?;
        let elapsed = micros(started.elapsed());
        self.metrics.minimum_write_micros = Some(
            self.metrics
                .minimum_write_micros
                .map_or(elapsed, |current| current.min(elapsed)),
        );
        self.metrics.maximum_write_micros = Some(
            self.metrics
                .maximum_write_micros
                .map_or(elapsed, |current| current.max(elapsed)),
        );
        self.metrics.total_write_micros = self.metrics.total_write_micros.saturating_add(elapsed);
        self.metrics.blocks_committed = self.metrics.blocks_committed.saturating_add(1);
        self.metrics.frames_committed = self
            .metrics
            .frames_committed
            .saturating_add(u64::from(header.frame_count));
        self.expected_start_frame = Some(
            header
                .start_frame
                .saturating_add(u64::from(header.frame_count)),
        );
        if self.metrics.blocks_committed == 1 {
            self.metrics.first_commit_micros =
                self.play_started.map(|start| micros(start.elapsed()));
            self.lifecycle = PlaybackLifecycle::FirstFrameCommitted;
        } else {
            self.lifecycle = PlaybackLifecycle::Active;
        }
        Ok(())
    }

    pub fn drain(&mut self) -> Result<(), PlaybackFailure> {
        if self.child.is_none() || self.lifecycle == PlaybackLifecycle::StoppedClosed {
            return Err(PlaybackFailure::InvalidLifecycle);
        }
        self.lifecycle = PlaybackLifecycle::DrainRequested;
        self.stdin.take();
        let status = self
            .child
            .as_mut()
            .ok_or(PlaybackFailure::InvalidLifecycle)?
            .wait()
            .map_err(|_| PlaybackFailure::DrainFailed)?;
        if !status.success() {
            return Err(self.classify_terminal_failure());
        }
        self.lifecycle = PlaybackLifecycle::Drained;
        self.child.take();
        self.stderr.take();
        self.lifecycle = PlaybackLifecycle::StoppedClosed;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), PlaybackFailure> {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            if child
                .try_wait()
                .map_err(|_| PlaybackFailure::CloseFailed)?
                .is_none()
            {
                child.kill().map_err(|_| PlaybackFailure::CloseFailed)?;
                child.wait().map_err(|_| PlaybackFailure::CloseFailed)?;
            }
        }
        self.stderr.take();
        self.lifecycle = PlaybackLifecycle::StoppedClosed;
        Ok(())
    }

    fn open(&mut self) -> Result<(), PlaybackFailure> {
        let current = discover_alsa_playback().map_err(|_| PlaybackFailure::ProviderLost)?;
        if !current.contains(&self.selection.observation) {
            self.lifecycle = PlaybackLifecycle::ProviderLost;
            return Err(PlaybackFailure::StaleObservation);
        }
        let target = self.selection.alsa_target();
        let mut child = Command::new("/usr/bin/aplay")
            .args([
                "--quiet",
                "--file-type=raw",
                "--format=S16_LE",
                "--rate=48000",
                "--channels=2",
                "--period-size=256",
                "--buffer-size=1024",
                "--disable-resample",
                "--disable-channels",
                "--disable-format",
                "--disable-softvol",
                "--fatal-errors",
                "--device",
                target.as_str(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| PlaybackFailure::OpenFailed)?;
        self.stdin = child.stdin.take();
        self.stderr = child.stderr.take();
        self.child = Some(child);
        self.lifecycle = PlaybackLifecycle::OpenedPrepared;
        self.play_started = Some(Instant::now());
        self.lifecycle = PlaybackLifecycle::Started;
        Ok(())
    }

    fn validate_header(&mut self, header: PcmFrameHeader) -> Result<(), PlaybackFailure> {
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.sample_rate_hz != SAMPLE_RATE_HZ
            || header.layout != PcmChannelLayout::StereoLeftRight
            || header.frame_count == 0
            || header.frame_count > PERIOD_FRAMES
            || header.clock_id != SOURCE_CLOCK_ID
        {
            return Err(PlaybackFailure::InvalidPcm);
        }
        if let Some(expected) = self.expected_start_frame {
            if header.start_frame != expected && !header.discontinuity {
                return Err(PlaybackFailure::DiscontinuousInput);
            }
            if header.discontinuity {
                self.metrics.discontinuities = self.metrics.discontinuities.saturating_add(1);
            }
        } else if header.discontinuity {
            self.metrics.discontinuities = 1;
        }
        Ok(())
    }

    fn child_exited(&mut self) -> Result<bool, PlaybackFailure> {
        self.child
            .as_mut()
            .ok_or(PlaybackFailure::InvalidLifecycle)?
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|_| PlaybackFailure::ProviderLost)
    }

    fn classify_terminal_failure(&mut self) -> PlaybackFailure {
        let mut diagnostic = [0_u8; MAXIMUM_DIAGNOSTIC_BYTES];
        let length = self
            .stderr
            .as_mut()
            .and_then(|stderr| stderr.read(&mut diagnostic).ok())
            .unwrap_or(0);
        let diagnostic = String::from_utf8_lossy(&diagnostic[..length]).to_ascii_lowercase();
        if diagnostic.contains("underrun") {
            self.metrics.underruns = self.metrics.underruns.saturating_add(1);
            self.lifecycle = PlaybackLifecycle::Underrun;
            PlaybackFailure::Underrun
        } else if diagnostic.contains("busy") {
            self.lifecycle = PlaybackLifecycle::Failed;
            PlaybackFailure::DeviceBusy
        } else if self.metrics.blocks_committed == 0 {
            self.lifecycle = PlaybackLifecycle::Failed;
            PlaybackFailure::OpenFailed
        } else {
            self.lifecycle = PlaybackLifecycle::ProviderLost;
            PlaybackFailure::ProviderLost
        }
    }
}

impl Drop for AlsaAplaySession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, OfferGeneration};

    fn selection() -> HostedPlaybackSelection {
        HostedPlaybackSelection::from_observation(
            super::super::AlsaPlaybackObservation {
                card_index: 7,
                card_id: "TEST".into(),
                card_name: "Test Card".into(),
                device: 2,
                device_name: "Test PCM".into(),
                base_identity: "pci-test".into(),
            },
            BootId::from("boot-test"),
            OfferGeneration(3),
        )
    }

    fn frame(start: u64, discontinuity: bool) -> Vec<u8> {
        PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            SAMPLE_RATE_HZ,
            PcmChannelLayout::StereoLeftRight,
            PERIOD_FRAMES,
            SOURCE_CLOCK_ID,
            start,
            discontinuity,
        )
        .unwrap()
        .encode_frame(&[0; PERIOD_FRAMES as usize * super::super::CHANNELS as usize * 2])
        .unwrap()
    }

    #[test]
    fn exact_pcm_and_continuity_are_validated_before_open() {
        let mut session = AlsaAplaySession::resolved(selection());
        assert_eq!(
            session.validate_header(PcmFrameHeader::decode_frame(&frame(0, false)).unwrap().0),
            Ok(())
        );
        session.expected_start_frame = Some(u64::from(PERIOD_FRAMES));
        assert_eq!(
            session.validate_header(PcmFrameHeader::decode_frame(&frame(999, false)).unwrap().0),
            Err(PlaybackFailure::DiscontinuousInput)
        );
        assert_eq!(
            session.validate_header(PcmFrameHeader::decode_frame(&frame(999, true)).unwrap().0),
            Ok(())
        );
        assert_eq!(session.metrics.discontinuities, 1);
        assert_eq!(session.lifecycle(), PlaybackLifecycle::ResolvedAvailable);
    }

    #[test]
    fn wrong_profile_refuses_before_open() {
        let mut encoded = frame(0, false);
        encoded[1..5].copy_from_slice(&44_100_u32.to_le_bytes());
        let header = PcmFrameHeader::decode_frame(&encoded).unwrap().0;
        let mut session = AlsaAplaySession::resolved(selection());
        assert_eq!(
            session.validate_header(header),
            Err(PlaybackFailure::InvalidPcm)
        );
        assert!(session.child.is_none());
    }
}
