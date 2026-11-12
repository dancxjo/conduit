//! Finite physical cue operation below an already admitted Host Call.
use super::{discover_alsa_playback, HostedPlaybackSelection};
use crate::{
    hosted_process::{run_process_with_input_bound, ProcessRequest, ProcessTerminal},
    RunControl,
};
use conduit_core::{InfoBool, PlannedGear};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use std::{ffi::OsString, path::Path, time::Duration};
pub(crate) const PCM_BYTES: usize = conduit_synth::STARTUP_CHIME_FRAMES as usize * 4;
pub(super) const ARGUMENTS: &[&str] = &[
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
];
pub(super) const WORKSPACE_DRIFT: &str =
    "startup chime retained workspace exceeded its derived term";
pub(super) const MISSING_SELECTION: &str = "startup chime has no checked selected speaker";
pub(super) const UNCHECKED_PROVIDER: &str = "startup chime requires a checked playback Base";
pub(super) const IDENTITY_MISMATCH: &str =
    "startup chime speaker identity/Boot/generation differs from selected Plan";

pub(crate) struct PreparedChimeOutput {
    selection: HostedPlaybackSelection,
    arguments: Vec<OsString>,
    attempted: bool,
}
impl PreparedChimeOutput {
    pub(crate) fn prepare(
        placement: &PlannedGear,
        selected: Option<&HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        let selected = selected.ok_or(MISSING_SELECTION)?;
        let expected_base = selected
            .checked_provider_binding()
            .ok_or(UNCHECKED_PROVIDER)?;
        if selected.boot_id != placement.boot_id
            || selected.offer_generation != placement.offer_generation
            || placement.base.as_ref() != Some(&expected_base)
            || placement.resources.len() != 1
            || placement.resources[0].pool_id != selected.pool_id()
        {
            return Err(IDENTITY_MISMATCH.into());
        }
        let mut arguments = Vec::with_capacity(ARGUMENTS.len() + 1);
        for value in ARGUMENTS {
            let mut argument = OsString::with_capacity(value.len());
            argument.push(value);
            arguments.push(argument);
        }
        // Qualified Linux OsString::from(String) transfers the byte Vec.
        arguments.push(OsString::from(selected.alsa_target()));
        let prepared = Self {
            selection: selected.clone(),
            arguments,
            attempted: false,
        };
        if let Some(derived) = super::startup_chime_workspace::derive(selected) {
            if prepared.retained_backing_bytes() > derived {
                return Err(WORKSPACE_DRIFT.into());
            }
        }
        Ok(prepared)
    }
    pub(crate) fn retained_backing_bytes(&self) -> usize {
        self.selection.backing_bytes()
            + self.arguments.capacity() * core::mem::size_of::<OsString>()
            + self.arguments.iter().map(OsString::capacity).sum::<usize>()
    }
    pub(crate) fn execute(
        &mut self,
        input: &[u8],
        pcm: &[u8],
        control: &RunControl,
    ) -> HostCallOutcome {
        self.execute_inner(
            input,
            pcm,
            &|| control.stop_requested(),
            #[cfg(test)]
            Some(control),
        )
    }
    pub(crate) fn execute_with_cancel(
        &mut self,
        input: &[u8],
        pcm: &[u8],
        cancelled: impl Fn() -> bool,
    ) -> HostCallOutcome {
        self.execute_inner(
            input,
            pcm,
            &cancelled,
            #[cfg(test)]
            None,
        )
    }
    fn execute_inner(
        &mut self,
        input: &[u8],
        pcm: &[u8],
        cancelled: &dyn Fn() -> bool,
        #[cfg(test)] test_control: Option<&RunControl>,
    ) -> HostCallOutcome {
        let Ok(pulse) = InfoBool::decode(input) else {
            return outcome(
                HostCallDisposition::Failed,
                FailureCode::HostCallFailed,
                481,
            );
        };
        if !pulse.get() {
            return completed();
        }
        if self.attempted || pcm.len() != PCM_BYTES {
            return outcome(
                HostCallDisposition::Failed,
                FailureCode::HostCallFailed,
                482,
            );
        }
        if cancelled() {
            return self::cancelled();
        }
        self.attempted = true;
        #[cfg(test)]
        if self.selection.fake_behavior.is_some() {
            return self.execute_fake(pcm, cancelled, test_control);
        }
        if self.selection.verify_executable_identity().is_err()
            || !discover_alsa_playback()
                .is_ok_and(|current| current.contains(&self.selection.observation))
        {
            return outcome(
                HostCallDisposition::Denied,
                FailureCode::HostCallDenied,
                483,
            );
        }
        let report = run_process_with_input_bound(
            &ProcessRequest {
                program: Path::new("/usr/bin/aplay"),
                arguments: &self.arguments,
                environment: &[("LC_ALL".into(), "C".into())],
                stdin: pcm,
                maximum_stdout_bytes: 0,
                maximum_stderr_bytes: 4096,
                timeout: Duration::from_millis(2500),
                require_process_group: true,
            },
            PCM_BYTES,
            cancelled,
        );
        match report {
            Ok(report) => match report.terminal {
                ProcessTerminal::Exited(status) if status.success() => completed(),
                ProcessTerminal::Cancelled => self::cancelled(),
                _ => outcome(
                    HostCallDisposition::Failed,
                    FailureCode::HostCallFailed,
                    484,
                ),
            },
            Err(_) => outcome(
                HostCallDisposition::Failed,
                FailureCode::HostCallFailed,
                485,
            ),
        }
    }
}
fn cancelled() -> HostCallOutcome {
    outcome(HostCallDisposition::Cancelled, FailureCode::Cancelled, 0)
}
fn completed() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: None,
        failure: None,
    }
}
fn outcome(disposition: HostCallDisposition, code: FailureCode, detail: u16) -> HostCallOutcome {
    HostCallOutcome {
        disposition,
        output: None,
        failure: Some(Failure { code, detail }),
    }
}

#[cfg(test)]
impl PreparedChimeOutput {
    fn execute_fake(
        &self,
        pcm: &[u8],
        cancelled: &dyn Fn() -> bool,
        test_control: Option<&RunControl>,
    ) -> HostCallOutcome {
        TEST_EFFECT_ATTEMPTS.with(|n| n.set(n.get() + 1));
        use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
        let Ok(mut session) = super::PlaybackSession::resolved(self.selection.clone()) else {
            return outcome(
                HostCallDisposition::Failed,
                FailureCode::HostCallFailed,
                486,
            );
        };
        let mut frame = [0u8; conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES as usize];
        for (index, block) in pcm
            .chunks(usize::from(super::PERIOD_FRAMES) * 4)
            .enumerate()
        {
            if cancelled() {
                let _ = session.stop();
                return self::cancelled();
            }
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                super::SAMPLE_RATE_HZ,
                PcmChannelLayout::StereoLeftRight,
                super::PERIOD_FRAMES,
                super::SOURCE_CLOCK_ID,
                index as u64 * u64::from(super::PERIOD_FRAMES),
                false,
            )
            .unwrap();
            frame[..conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
            frame[conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN..].copy_from_slice(block);
            if let Err(error) = session.write_frame(&frame) {
                return match error {
                    super::PlaybackFailure::DeviceBusy
                    | super::PlaybackFailure::StaleObservation => outcome(
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
            if index == 0 && TEST_CANCEL_AFTER_FIRST_BLOCK.with(|n| n.replace(false)) {
                if let Some(control) = test_control {
                    control
                        .request_stop(
                            crate::RunControlRequestId::new("fixture/cancel-during-cue").unwrap(),
                        )
                        .unwrap();
                }
                let _ = session.stop();
                return self::cancelled();
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
    static TEST_CANCEL_AFTER_FIRST_BLOCK:core::cell::Cell<bool>=const {core::cell::Cell::new(false)};
}
#[cfg(test)]
pub(crate) fn cancel_after_first_test_block() {
    TEST_CANCEL_AFTER_FIRST_BLOCK.with(|n| n.set(true));
}
#[cfg(test)]
pub(crate) fn reset_test_attempts() {
    TEST_EFFECT_ATTEMPTS.with(|n| n.set(0));
    TEST_CANCEL_AFTER_FIRST_BLOCK.with(|n| n.set(false));
}
#[cfg(test)]
pub(crate) fn test_attempts() -> usize {
    TEST_EFFECT_ATTEMPTS.with(core::cell::Cell::get)
}
