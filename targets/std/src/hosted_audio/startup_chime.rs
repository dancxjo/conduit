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
        let selected = selected.ok_or("startup chime has no checked selected speaker")?;
        let base = selected
            .checked_base_entry()
            .ok_or("startup chime requires a checked playback Base")?;
        if selected.boot_id != placement.boot_id
            || selected.offer_generation != placement.offer_generation
            || placement.base.as_ref() != Some(&base.binding())
            || placement.resources.len() != 1
            || placement.resources[0].pool_id != selected.pool_id()
            || placement.realization_characteristics
                != selected
                    .realization_advertisement(placement.host_id.clone())
                    .characteristics
        {
            return Err(
                "startup chime speaker identity/Boot/generation differs from selected Plan".into(),
            );
        }
        let arguments = [
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
        ]
        .into_iter()
        .map(OsString::from)
        .chain(std::iter::once(OsString::from(selected.alsa_target())))
        .collect();
        Ok(Self {
            selection: selected.clone(),
            arguments,
            attempted: false,
        })
    }
    pub(crate) fn retained_backing_bytes(&self) -> usize {
        let o = &self.selection.observation;
        self.selection.boot_id.0.capacity()
            + o.card_id.capacity()
            + o.card_name.capacity()
            + o.device_name.capacity()
            + o.base_identity.capacity()
            + self.arguments.capacity() * core::mem::size_of::<OsString>()
            + self.arguments.iter().map(OsString::capacity).sum::<usize>()
    }
    pub(crate) fn execute(
        &mut self,
        input: &[u8],
        pcm: &[u8],
        control: &RunControl,
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
        self.attempted = true;
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
            || control.requested_stop().is_some(),
        );
        match report {
            Ok(report) => match report.terminal {
                ProcessTerminal::Exited(status) if status.success() => completed(),
                ProcessTerminal::Cancelled => HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: Some(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                },
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
