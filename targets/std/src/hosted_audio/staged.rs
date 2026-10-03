//! Selected, finite hosted playback timing stage. The kernel still owns the
//! ordered PCM Host Calls; this worker only clocks admitted blocks to ALSA.
use super::{
    HostedPlaybackSelection, PlaybackFailure, PlaybackLifecycle, PlaybackReport, PlaybackSession,
    PERIOD_FRAMES, SAMPLE_RATE_HZ, SOURCE_CLOCK_ID, SPOKEN_QUEUE_BLOCKS, SPOKEN_QUEUE_BYTES,
    SPOKEN_QUEUE_FRAMES, SPOKEN_START_FRAMES,
};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

const MAXIMUM_TOTAL_BLOCKS: usize =
    conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS as usize;
const MAXIMUM_BLOCK_BYTES: usize =
    conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES as usize;
const _: () = {
    assert!(MAXIMUM_TOTAL_BLOCKS > SPOKEN_QUEUE_BLOCKS);
    assert!(SPOKEN_QUEUE_BYTES as usize == SPOKEN_QUEUE_BLOCKS * MAXIMUM_BLOCK_BYTES);
};

struct Block {
    bytes: [u8; MAXIMUM_BLOCK_BYTES],
    length: u16,
    frames: u16,
}

struct Queue {
    blocks: VecDeque<Block>,
    queued_frames: u32,
    started: bool,
    input_closed: bool,
    stop_requested: bool,
    failure: Option<PlaybackFailure>,
    #[cfg(test)]
    hold_clock: bool,
    #[cfg(test)]
    waiting_writers: u32,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;
type WorkerResult = (Box<PlaybackSession>, Result<(), PlaybackFailure>);

pub(crate) struct StagedPlaybackSession {
    selection: HostedPlaybackSelection,
    shared: Shared,
    worker: Option<JoinHandle<WorkerResult>>,
    report: Option<PlaybackReport>,
    expected_start_frame: Option<u64>,
    total_frames: u64,
    blocks_accepted: u32,
    closed: bool,
}

impl StagedPlaybackSession {
    pub(super) fn prepare(selection: HostedPlaybackSelection) -> Result<Self, String> {
        let mut blocks = VecDeque::new();
        blocks
            .try_reserve_exact(SPOKEN_QUEUE_BLOCKS)
            .map_err(|_| "cannot admit bounded spoken playback queue".to_string())?;
        let shared = Arc::new((
            Mutex::new(Queue {
                blocks,
                queued_frames: 0,
                started: false,
                input_closed: false,
                stop_requested: false,
                failure: None,
                #[cfg(test)]
                hold_clock: false,
                #[cfg(test)]
                waiting_writers: 0,
            }),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let device = Box::new(PlaybackSession::resolved_direct(selection.clone()));
        let worker = thread::Builder::new()
            .name("conduit-bounded-speech-playback".into())
            .spawn(move || clock_device(worker_shared, device))
            .map_err(|error| format!("cannot admit spoken playback worker: {error}"))?;
        Ok(Self {
            selection,
            shared,
            worker: Some(worker),
            report: None,
            expected_start_frame: None,
            total_frames: 0,
            blocks_accepted: 0,
            closed: false,
        })
    }

    pub(super) fn write_frame(&mut self, encoded: &[u8]) -> Result<(), PlaybackFailure> {
        if self.closed {
            return Err(PlaybackFailure::InvalidLifecycle);
        }
        let (header, _) =
            PcmFrameHeader::decode_frame(encoded).map_err(|_| PlaybackFailure::InvalidPcm)?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.sample_rate_hz != SAMPLE_RATE_HZ
            || header.layout != PcmChannelLayout::StereoLeftRight
            || header.frame_count == 0
            || header.frame_count > PERIOD_FRAMES
            || header.clock_id != SOURCE_CLOCK_ID
            || encoded.len() > MAXIMUM_BLOCK_BYTES
            || self
                .expected_start_frame
                .is_some_and(|expected| expected != header.start_frame && !header.discontinuity)
        {
            return Err(PlaybackFailure::InvalidPcm);
        }
        let next_start_frame = header
            .start_frame
            .checked_add(u64::from(header.frame_count))
            .ok_or(PlaybackFailure::InvalidPcm)?;
        let next_total = self.total_frames + u64::from(header.frame_count);
        if next_total > 16_384 * u64::from(SAMPLE_RATE_HZ) / 1000
            || self.blocks_accepted >= MAXIMUM_TOTAL_BLOCKS as u32
        {
            return Err(PlaybackFailure::StagingExceeded);
        }
        let (lock, ready) = &*self.shared;
        let mut queue = lock.lock().map_err(|_| PlaybackFailure::WriteFailed)?;
        while queue.blocks.len() == SPOKEN_QUEUE_BLOCKS
            || queue.queued_frames + u32::from(header.frame_count) > SPOKEN_QUEUE_FRAMES
        {
            queue.started = true;
            #[cfg(test)]
            {
                queue.waiting_writers += 1;
            }
            ready.notify_all();
            queue = ready
                .wait(queue)
                .map_err(|_| PlaybackFailure::WriteFailed)?;
            #[cfg(test)]
            {
                queue.waiting_writers -= 1;
            }
            if let Some(failure) = queue.failure {
                return Err(failure);
            }
            if queue.stop_requested {
                return Err(PlaybackFailure::InvalidLifecycle);
            }
        }
        if let Some(failure) = queue.failure {
            return Err(failure);
        }
        let mut block = Block {
            bytes: [0; MAXIMUM_BLOCK_BYTES],
            length: encoded.len() as u16,
            frames: header.frame_count,
        };
        block.bytes[..encoded.len()].copy_from_slice(encoded);
        queue.blocks.push_back(block);
        queue.queued_frames += u32::from(header.frame_count);
        if queue.queued_frames >= SPOKEN_START_FRAMES {
            queue.started = true;
        }
        ready.notify_all();
        self.expected_start_frame = Some(next_start_frame);
        self.total_frames = next_total;
        self.blocks_accepted += 1;
        Ok(())
    }

    pub(super) fn drain(&mut self) -> Result<(), PlaybackFailure> {
        if self.closed || self.blocks_accepted == 0 {
            return Err(PlaybackFailure::InvalidLifecycle);
        }
        self.closed = true;
        let (lock, ready) = &*self.shared;
        {
            let mut queue = lock.lock().map_err(|_| PlaybackFailure::DrainFailed)?;
            queue.input_closed = true;
            queue.started = true;
            ready.notify_all();
        }
        self.join()
    }

    pub(super) fn stop(&mut self) -> Result<(), PlaybackFailure> {
        if self.worker.is_none() {
            return Ok(());
        }
        self.closed = true;
        let (lock, ready) = &*self.shared;
        {
            let mut queue = lock.lock().map_err(|_| PlaybackFailure::CloseFailed)?;
            queue.stop_requested = true;
            queue.blocks.clear();
            queue.queued_frames = 0;
            ready.notify_all();
        }
        self.join()
    }

    fn join(&mut self) -> Result<(), PlaybackFailure> {
        let Some(worker) = self.worker.take() else {
            return Err(PlaybackFailure::InvalidLifecycle);
        };
        let (device, result) = worker.join().map_err(|_| PlaybackFailure::ProviderLost)?;
        let mut report = device.report();
        report.backend = if report.backend == "deterministic-playback-fixture@1" {
            "deterministic-bounded-speech-fixture@1"
        } else {
            "alsa-aplay-bounded-speech@1"
        };
        report.controlled_staging_bytes = SPOKEN_QUEUE_BYTES;
        report.timing_class = "finite-staged-hosted-best-effort";
        self.report = Some(report);
        result
    }

    pub(super) fn lifecycle(&self) -> PlaybackLifecycle {
        self.report
            .as_ref()
            .map_or(PlaybackLifecycle::ResolvedAvailable, |report| {
                report.lifecycle
            })
    }

    pub(super) fn report(&self) -> PlaybackReport {
        self.report.clone().unwrap_or_else(|| {
            let mut report = super::AlsaAplaySession::resolved(self.selection.clone()).report();
            report.backend = "alsa-aplay-bounded-speech@1";
            report.controlled_staging_bytes = SPOKEN_QUEUE_BYTES;
            report.timing_class = "finite-staged-hosted-best-effort";
            report
        })
    }
}

impl Drop for StagedPlaybackSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn clock_device(shared: Shared, mut device: Box<PlaybackSession>) -> WorkerResult {
    loop {
        let next = {
            let (lock, ready) = &*shared;
            let mut queue = match lock.lock() {
                Ok(queue) => queue,
                Err(_) => return (device, Err(PlaybackFailure::ProviderLost)),
            };
            loop {
                if queue.stop_requested {
                    let outcome = device.stop();
                    return (device, outcome);
                }
                #[cfg(test)]
                if queue.hold_clock {
                    queue = match ready.wait(queue) {
                        Ok(queue) => queue,
                        Err(_) => return (device, Err(PlaybackFailure::ProviderLost)),
                    };
                    continue;
                }
                if queue.started {
                    if let Some(block) = queue.blocks.pop_front() {
                        queue.queued_frames -= u32::from(block.frames);
                        ready.notify_all();
                        break Some(block);
                    }
                    if queue.input_closed {
                        let outcome = device.drain();
                        return (device, outcome);
                    }
                }
                queue = match ready.wait(queue) {
                    Ok(queue) => queue,
                    Err(_) => return (device, Err(PlaybackFailure::ProviderLost)),
                };
            }
        };
        if let Some(block) = next {
            if let Err(failure) = device.write_frame(&block.bytes[..usize::from(block.length)]) {
                let (lock, ready) = &*shared;
                if let Ok(mut queue) = lock.lock() {
                    queue.failure = Some(failure);
                    ready.notify_all();
                }
                return (device, Err(failure));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_audio::{AlsaPlaybackObservation, FakePlaybackBehavior};
    use conduit_core::{BootId, OfferGeneration};

    fn selection(behavior: FakePlaybackBehavior) -> HostedPlaybackSelection {
        HostedPlaybackSelection::deterministic_fake(
            AlsaPlaybackObservation {
                card_index: 1,
                card_id: "BOUNDED_TEST".into(),
                card_name: "Bounded fixture".into(),
                device: 0,
                device_name: "Fixture sink".into(),
                base_identity: "bounded-fixture".into(),
            },
            BootId::from("boot/bounded-fixture"),
            OfferGeneration(1),
            behavior,
        )
        .with_bounded_speech_queue()
    }

    fn frame_at(start_frame: u64) -> Vec<u8> {
        PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            SAMPLE_RATE_HZ,
            PcmChannelLayout::StereoLeftRight,
            PERIOD_FRAMES,
            SOURCE_CLOCK_ID,
            start_frame,
            false,
        )
        .unwrap()
        .encode_frame(&[0; PERIOD_FRAMES as usize * 4])
        .unwrap()
    }

    fn frame() -> Vec<u8> {
        frame_at(0)
    }

    #[test]
    fn below_lead_pcm_waits_for_input_close_and_drains_to_selected_sink() {
        let mut session =
            StagedPlaybackSession::prepare(selection(FakePlaybackBehavior::Success)).unwrap();
        session.write_frame(&frame()).unwrap();
        assert_eq!(session.report().metrics.blocks_committed, 0);
        assert_eq!(session.lifecycle(), PlaybackLifecycle::ResolvedAvailable);
        session.drain().unwrap();
        let report = session.report();
        assert_eq!(report.metrics.blocks_committed, 1);
        assert_eq!(report.metrics.frames_committed, u64::from(PERIOD_FRAMES));
        assert_eq!(report.controlled_staging_bytes, SPOKEN_QUEUE_BYTES);
        assert_eq!(report.lifecycle, PlaybackLifecycle::StoppedClosed);
    }

    #[test]
    fn stop_before_lead_discards_queued_pcm_without_opening_sink() {
        let mut session =
            StagedPlaybackSession::prepare(selection(FakePlaybackBehavior::Success)).unwrap();
        session.write_frame(&frame()).unwrap();
        session.stop().unwrap();
        assert_eq!(session.report().metrics.blocks_committed, 0);
        assert_eq!(session.drain(), Err(PlaybackFailure::InvalidLifecycle));
    }

    #[test]
    fn selected_provider_loss_at_drain_is_not_playback_completion() {
        let mut session = StagedPlaybackSession::prepare(selection(
            FakePlaybackBehavior::ProviderLossOnFirstBlock,
        ))
        .unwrap();
        session.write_frame(&frame()).unwrap();
        assert_eq!(session.drain(), Err(PlaybackFailure::ProviderLost));
        assert_eq!(session.report().metrics.frames_committed, 0);
    }

    #[test]
    fn overflowing_source_position_is_refused_before_queueing_pcm() {
        let mut session =
            StagedPlaybackSession::prepare(selection(FakePlaybackBehavior::Success)).unwrap();
        assert_eq!(
            session.write_frame(&frame_at(u64::MAX)),
            Err(PlaybackFailure::InvalidPcm)
        );
        let (lock, _) = &*session.shared;
        let queue = lock.lock().unwrap();
        assert!(queue.blocks.is_empty());
        assert_eq!(queue.queued_frames, 0);
        drop(queue);
        assert_eq!(session.blocks_accepted, 0);
        assert_eq!(session.expected_start_frame, None);
        assert_eq!(session.report().metrics.blocks_committed, 0);
    }

    #[test]
    fn full_queue_applies_pressure_without_growing_or_dropping_ordered_pcm() {
        let mut session =
            StagedPlaybackSession::prepare(selection(FakePlaybackBehavior::Success)).unwrap();
        let shared = Arc::clone(&session.shared);
        {
            let (lock, _) = &*shared;
            lock.lock().unwrap().hold_clock = true;
        }
        let producer = thread::spawn(move || {
            for index in 0..376_u64 {
                session.write_frame(&frame_at(index * u64::from(PERIOD_FRAMES)))?;
            }
            session.drain()?;
            Ok::<_, PlaybackFailure>(session.report())
        });
        let (lock, ready) = &*shared;
        let mut queue = lock.lock().unwrap();
        while queue.waiting_writers == 0 {
            queue = ready.wait(queue).unwrap();
        }
        assert_eq!(queue.queued_frames, SPOKEN_QUEUE_FRAMES);
        assert_eq!(queue.blocks.len(), 375);
        queue.hold_clock = false;
        ready.notify_all();
        drop(queue);
        let report = producer.join().unwrap().unwrap();
        assert_eq!(report.metrics.blocks_committed, 376);
        assert_eq!(
            report.metrics.frames_committed,
            376 * u64::from(PERIOD_FRAMES)
        );
    }
}
