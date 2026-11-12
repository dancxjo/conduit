//! Installed-owner Text file execution through the original Body lifecycle.
//! Callers select/plan the source and generation; this runner supplies neither.
use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyLifecycleRealization, ResidentPlot};
use conduit_core::*;
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[path = "text_file_plan.rs"]
mod planning;

struct OneOutput {
    expected: PortId,
    bytes: Vec<u8>,
    delivered: bool,
}
impl OneOutput {
    fn prepare(write: bool) -> Self {
        Self {
            expected: port_id(if write { "saved" } else { "value" }),
            bytes: Vec::with_capacity(conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize),
            delivered: false,
        }
    }
}
impl BodyForeOutputAdapter for OneOutput {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        if self.delivered
            || value.bytes.len() > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize
            || value.front_port_id != self.expected
            || value.track != ConnectionTrack::Payload
        {
            return Err("Text file Play exceeded its exact prepared output".into());
        }
        self.bytes.extend_from_slice(&value.bytes);
        self.delivered = true;
        Ok(())
    }
}

/// Process-local outcome from the actual installed kernel, not a durable receipt.
/// The caller must retain terminal lifecycle evidence and its own exact content
/// selection before claiming publication or acknowledging a remote producer.
pub(super) struct TextFilePlayOutcome {
    pub(super) proposed: BodyLifecycleRealization,
    pub(super) host: HostAdvertisement,
    pub(super) report: BodyRunReport,
    pub(super) output: Option<Vec<u8>>,
}
impl Owner {
    /// Execute an already selected, proposed file operation on this SAME Host.
    /// No replacement Body, alternate scheduler or direct filesystem write is
    /// constructed. Failed terminal reports remain available to the caller.
    pub(super) fn run_selected_text_file_plan(
        &mut self,
        root: &Path,
        write: bool,
        bytes: Vec<u8>,
    ) -> Result<TextFilePlayOutcome, String> {
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .ok_or("selected Text file Plan required")?
            .clone();
        let ad = self.host.advertisement().clone();
        let inputs = [ExternalForeInput {
            front_port_id: port_id(if write { "value" } else { "selected" }),
            track: ConnectionTrack::Payload,
            bytes,
        }];
        let control = RunControl::default();
        let session = &mut self.session;
        let admissions = self.admissions.as_ref();
        let host = &mut self.host;
        let (report, output) = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("conduit-text-file".into())
                .stack_size(4 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    let mut fore = OneOutput::prepare(write);
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_secs(5),
                        control: control.clone(),
                    };
                    let report = host.run_body_plan_with_fore_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        &inputs,
                        false,
                        &mut fore,
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let mut next = session.clone();
                            next.started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                                .map_err(debug)?;
                            state::retain_session(root, &mut next, None, admissions, None, None)?;
                            *session = next;
                            Ok(())
                        },
                    );
                    (report, fore)
                })
                .map_err(|error| format!("start Text file worker: {error}"))?
                .join()
                .map_err(|_| "Text file worker panicked".to_string())
        })?;
        let report = report?;
        let success = report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && report.fore_deliveries.len() == 1
            && output.delivered
            && report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone());

        Ok(TextFilePlayOutcome {
            proposed,
            host: ad,
            report,
            output: success.then_some(output.bytes),
        })
    }
}

#[cfg(test)]
#[path = "text_file_play_tests.rs"]
mod tests;
