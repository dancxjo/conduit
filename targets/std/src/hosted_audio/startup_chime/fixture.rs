//! Existing deterministic playback backend, prepared before execution.
use super::*;
#[cfg(test)]
impl PreparedChimeOutput {
    pub(super) fn execute_fixture(
        &mut self,
        pcm: &[u8],
        cancelled: &dyn Fn() -> bool,
        test_control: Option<&RunControl>,
    ) -> HostCallOutcome {
        TEST_EFFECT_ATTEMPTS.with(|n| n.set(n.get() + 1));
        use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
        let session = self.fixture.as_mut().expect("prepared fixture session");
        let mut frame = [0u8; conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES as usize];
        for (index, block) in pcm
            .chunks(usize::from(crate::hosted_audio::PERIOD_FRAMES) * 4)
            .enumerate()
        {
            if cancelled() {
                let _ = session.stop();
                return cancelled_outcome();
            }
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                crate::hosted_audio::SAMPLE_RATE_HZ,
                PcmChannelLayout::StereoLeftRight,
                crate::hosted_audio::PERIOD_FRAMES,
                crate::hosted_audio::SOURCE_CLOCK_ID,
                index as u64 * u64::from(crate::hosted_audio::PERIOD_FRAMES),
                false,
            )
            .unwrap();
            frame[..conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
            frame[conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN..].copy_from_slice(block);
            if let Err(error) = session.write_frame(&frame) {
                return match error {
                    crate::hosted_audio::PlaybackFailure::DeviceBusy
                    | crate::hosted_audio::PlaybackFailure::StaleObservation => outcome(
                        HostCallDisposition::Denied,
                        FailureCode::HostCallDenied,
                        487,
                    ),
                    _ => outcome(
                        HostCallDisposition::Failed,
                        FailureCode::HostCallFailed,
                        488,
                    ),
                };
            }
            let cancellation = if index == 0 {
                TEST_CANCEL_AFTER_FIRST_BLOCK.with(|request| request.borrow_mut().take())
            } else {
                None
            };
            if let Some(request) = cancellation {
                if let Some(control) = test_control {
                    control.request_stop(request).unwrap();
                }
                let _ = session.stop();
                return cancelled_outcome();
            }
        }
        if session.drain().and_then(|_| session.stop()).is_err() {
            return outcome(
                HostCallDisposition::Failed,
                FailureCode::HostCallFailed,
                489,
            );
        }
        completed()
    }
}

#[cfg(test)]
thread_local! {
    static TEST_EFFECT_ATTEMPTS:core::cell::Cell<usize>=const {core::cell::Cell::new(0)};
    // Prepared by the fixture caller, just like product-owned RunControl input;
    // its String never belongs to the funded kernel/provider receipt.
    static TEST_CANCEL_AFTER_FIRST_BLOCK:core::cell::RefCell<Option<crate::RunControlRequestId>>=const {core::cell::RefCell::new(None)};
}
#[cfg(test)]
pub(crate) fn cancel_after_first_test_block() {
    let request =
        crate::RunControlRequestId::new("fixture/cancel-during-cue").expect("fixed caller request");
    TEST_CANCEL_AFTER_FIRST_BLOCK.with(|slot| *slot.borrow_mut() = Some(request));
}
#[cfg(test)]
pub(crate) fn reset_test_attempts() {
    TEST_EFFECT_ATTEMPTS.with(|n| n.set(0));
    TEST_CANCEL_AFTER_FIRST_BLOCK.with(|slot| slot.borrow_mut().take());
}
pub(crate) fn test_attempts() -> usize {
    TEST_EFFECT_ATTEMPTS.with(core::cell::Cell::get)
}
