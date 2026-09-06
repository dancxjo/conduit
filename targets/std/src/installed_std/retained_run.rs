//! Keep owned State separate from cloneable execution reports.
use super::{InstalledOperation, InstalledScheduler};
#[cfg(test)]
use super::{RunControl, TimerAdapter};
use crate::{state_value::RetainedStdRun, StdRunReport};
use conduit_core::HostAdvertisement;
#[cfg(test)]
use conduit_core::PlanFragment;
#[cfg(test)]
use std::io::Write;

pub(crate) struct InstalledRunHost<'a, 'keyboard, 'model> {
    pub advertisement: &'a HostAdvertisement,
    pub playback: Option<&'a crate::hosted_audio::HostedPlaybackSelection>,
    pub midi_input: Option<&'a crate::hosted_midi::HostedRawMidiSelection>,
    pub midi_output: Option<&'a crate::hosted_midi::MidiOutputSelection>,
    pub keyboard: Option<&'keyboard mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
    pub local_model:
        Option<&'model mut (dyn crate::hosted_local_model::HostedLocalModelAdapter + 'static)>,
    pub vector_search:
        Option<&'model mut (dyn crate::hosted_vector_search::HostedVectorSearchAdapter + 'static)>,
    pub calendar: Option<&'model mut (dyn crate::hosted_calendar::HostedCalendarAdapter + 'static)>,
}

#[cfg(test)]
pub(crate) fn run_fragment<W: Write, T: TimerAdapter>(
    host: InstalledRunHost<'_, '_, '_>,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_sign_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
    control: &RunControl,
) -> Result<StdRunReport, String> {
    super::run_fragment_retaining(
        host,
        fragment,
        play_sequence,
        next_sign_sequence,
        output,
        timer,
        control,
    )
    .map(|run| run.report)
}

pub(super) fn finish(
    report: StdRunReport,
    scheduler: InstalledScheduler,
    state_count: usize,
) -> Result<RetainedStdRun, String> {
    let mut states = Vec::with_capacity(state_count);
    if state_count != 0 {
        let retired = scheduler.try_retire().map_err(|_| {
            "State execution is not terminal and drained; continuity unavailable".to_string()
        })?;
        for driver in retired.drivers.into_iter().take(retired.active_nodes) {
            if let InstalledOperation::TypedState(operation) = driver.into_operation() {
                let state = operation
                    .try_retire()
                    .map_err(|failure| format!("retire typed State: {}", failure.reason))?;
                states.push(state);
            }
        }
        if states.len() != state_count {
            return Err("retired State ownership differs from the sealed count".into());
        }
    }
    Ok(RetainedStdRun { report, states })
}
