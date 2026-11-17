//! One finite selected cue below the original admitted Host Call.
use super::{selected_identity::SelectedIdentity, HostedPlaybackSelection};
use crate::{
    hosted_process::{PreparedProcess, ProcessRequest, ProcessTerminal},
    RunControl,
};
use conduit_core::{InfoBool, PlannedGear};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use std::{ffi::OsString, path::Path, time::Duration};
pub(crate) mod layout;
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

pub(crate) struct PreparedChimeOutput {
    selection: HostedPlaybackSelection,
    metadata: Option<PreparedProcess>,
    playback: Option<PreparedProcess>,
    identity: SelectedIdentity,
    attempted: bool,
    #[cfg(test)]
    fixture: Option<super::fake::FakePlaybackSession>,
}
impl PreparedChimeOutput {
    pub(crate) fn prepare(
        placement: &PlannedGear,
        selected: Option<&HostedPlaybackSelection>,
        pcm: &[u8],
    ) -> Result<Self, String> {
        let selected = selected.ok_or("startup chime has no checked selected speaker")?;
        let base = selected
            .checked_provider_binding()
            .ok_or("startup chime requires checked playback Base")?;
        if selected.boot_id != placement.boot_id
            || selected.offer_generation != placement.offer_generation
            || placement.base.as_ref() != Some(&base)
            || placement.resources.len() != 1
            || placement.resources[0].pool_id != selected.pool_id()
            || pcm.len() != PCM_BYTES
        {
            return Err("startup chime differs from exact selected Plan/speaker/PCM".into());
        }
        let mut arguments = Vec::with_capacity(ARGUMENTS.len() + 1);
        for value in ARGUMENTS {
            let mut argument = OsString::with_capacity(value.len());
            argument.push(value);
            arguments.push(argument);
        }
        arguments.push(OsString::from(selected.alsa_target()));
        let environment = [("LC_ALL".into(), "C".into())];
        let metadata = PreparedProcess::prepare_funded(
            &ProcessRequest {
                program: Path::new("/usr/bin/aplay"),
                arguments: &["--playback".into(), "-l".into()],
                environment: &environment,
                stdin: &[],
                maximum_stdout_bytes: 64 * 1024,
                maximum_stderr_bytes: 4096,
                timeout: Duration::from_secs(5),
                require_process_group: true,
            },
            0,
        )
        .map_err(|_| "startup metadata process preparation refused")?;
        let playback = PreparedProcess::prepare_funded(
            &ProcessRequest {
                program: Path::new("/usr/bin/aplay"),
                arguments: &arguments,
                environment: &environment,
                stdin: pcm,
                maximum_stdout_bytes: 0,
                maximum_stderr_bytes: 4096,
                timeout: Duration::from_millis(2500),
                require_process_group: true,
            },
            PCM_BYTES,
        )
        .map_err(|_| "startup playback process preparation refused")?;
        Ok(Self {
            selection: selected.clone(),
            metadata: Some(metadata),
            playback: Some(playback),
            identity: SelectedIdentity::prepare(selected.observation.card_index)?,
            attempted: false,
            #[cfg(test)]
            fixture: selected
                .fake_behavior
                .map(|behavior| super::fake::FakePlaybackSession::new(selected.clone(), behavior)),
        })
    }
    pub(crate) fn retained_backing_bytes(&self) -> usize {
        self.selection.backing_bytes()
            + self.identity.retained_backing_bytes()
            + self
                .metadata
                .as_ref()
                .map_or(0, PreparedProcess::reserved_backing_bytes)
            + self
                .playback
                .as_ref()
                .map_or(0, PreparedProcess::reserved_backing_bytes)
            + if cfg!(test) {
                self.selection.backing_bytes()
            } else {
                0
            }
    }
    pub(crate) fn execute_controlled(
        &mut self,
        input: &[u8],
        pcm: &[u8],
        _control: &RunControl,
        cancelled: impl Fn() -> bool,
    ) -> HostCallOutcome {
        self.execute_inner(
            input,
            pcm,
            &cancelled,
            #[cfg(test)]
            Some(_control),
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
            return failed(481);
        };
        if !pulse.get() {
            return completed();
        }
        if self.attempted || pcm.len() != PCM_BYTES {
            return failed(482);
        }
        if cancelled() {
            return cancelled_outcome();
        }
        self.attempted = true;
        #[cfg(test)]
        if self.fixture.is_some() {
            return self.execute_fixture(pcm, cancelled, test_control);
        }
        if self.selection.verify_executable_identity().is_err() {
            return denied(483);
        }
        let Some(metadata) = self.metadata.take() else {
            return failed(482);
        };
        let metadata = match metadata.run(&[], cancelled) {
            Ok(report) => report,
            Err(_) => return denied(483),
        };
        if matches!(metadata.terminal, ProcessTerminal::Cancelled) {
            return cancelled_outcome();
        }
        if !matches!(metadata.terminal,ProcessTerminal::Exited(status) if status.success())
            || metadata.stdout.observed_bytes > 64 * 1024
            || metadata.stderr.observed_bytes > 4096
        {
            return denied(483);
        }
        let Ok(listing) = std::str::from_utf8(&metadata.stdout.retained) else {
            return denied(483);
        };
        let expected = &self.selection.observation;
        let mut found = false;
        for row in super::discovery_rows::playback_rows(listing) {
            let Ok(row) = row else {
                return denied(483);
            };
            if row.card_index == expected.card_index
                && row.card_id == expected.card_id
                && row.card_name == expected.card_name
                && row.device == expected.device
                && row.device_name == expected.device_name
                && self.identity.matches(&expected.base_identity)
            {
                found = true;
            }
        }
        if !found {
            return denied(483);
        }
        let Some(playback) = self.playback.take() else {
            return failed(482);
        };
        match playback.run(pcm, cancelled) {
            Ok(report) => match report.terminal {
                ProcessTerminal::Exited(status) if status.success() => completed(),
                ProcessTerminal::Cancelled => cancelled_outcome(),
                _ => failed(484),
            },
            Err(_) => failed(485),
        }
    }
}
fn cancelled_outcome() -> HostCallOutcome {
    outcome(HostCallDisposition::Cancelled, FailureCode::Cancelled, 0)
}
fn completed() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: None,
        failure: None,
    }
}
fn failed(detail: u16) -> HostCallOutcome {
    outcome(
        HostCallDisposition::Failed,
        FailureCode::HostCallFailed,
        detail,
    )
}
fn denied(detail: u16) -> HostCallOutcome {
    outcome(
        HostCallDisposition::Denied,
        FailureCode::HostCallDenied,
        detail,
    )
}
fn outcome(disposition: HostCallDisposition, code: FailureCode, detail: u16) -> HostCallOutcome {
    HostCallOutcome {
        disposition,
        output: None,
        failure: Some(Failure { code, detail }),
    }
}
#[cfg(test)]
mod fixture;
#[cfg(test)]
pub(crate) use fixture::{cancel_after_first_test_block, reset_test_attempts, test_attempts};
