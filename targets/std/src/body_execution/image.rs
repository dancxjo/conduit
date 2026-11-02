//! Selected IMAGE result custody. Escaped immutable Fore payload clones retain
//! the shared byte charge; borrowing cannot move other report backing.
use super::*;
use crate::kernel_preparation::ImageMemoryLease;

pub struct ImageBodyRunReport {
    report: BodyRunReport,
    _memory: ImageMemoryLease,
}
impl ImageBodyRunReport {
    pub fn report(&self) -> &BodyRunReport {
        &self.report
    }
}
impl StdHost {
    /// Direct cooperative tick-only execution. The result retains its byte
    /// charge until this wrapper and any escaped Fore payload clones drop; timer slots
    /// retire independently. No raw-report conversion is supplied.
    pub fn run_selected_image_body_plan_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_selected_image_body_plan_with_output_to_with_start(
            request, output, timer, None, started,
        )
    }
    /// Same selected custody, with the ordinary sealed finite Fore output.
    /// The caller adapter's own backing/effects remain independently admitted.
    pub fn run_selected_image_body_plan_with_output_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        fore: Option<&mut dyn BodyForeOutputAdapter>,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if !self.kernel_resources.has_image_bounds() {
            return Err("selected IMAGE execution requires IMAGE-bound Host".into());
        }
        let mut custody = None;
        let report = self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            fore.map(|adapter| (&[][..], false, adapter)),
            None,
            None,
            None,
            None,
            started,
            Some(&mut custody),
        )?;
        Ok(ImageBodyRunReport {
            report,
            _memory: custody.ok_or("selected IMAGE result lost its backing custody")?,
        })
    }
}

pub(super) fn prepare_lifecycle(
    request: &BodyRunRequest<'_>,
    play: &BodyPlayIdentity,
    host: &conduit_core::HostAdvertisement,
) -> Result<(Wake, SignIdentity), String> {
    let sign = |sequence| {
        bind_sign(
            &host.host_id,
            &host.boot_id,
            Some(&play.active_play_id),
            sequence,
        )
    };
    let wake = request
        .wake
        .body_plan_ready(request.plan, sign(0).sign_id)
        .and_then(|wake| wake.body_play_started(request.plan, play, sign(1).sign_id))
        .map_err(|error| format!("Body start lifecycle: {error:?}"))?;
    Ok((wake, sign(2)))
}

#[cfg(test)]
mod tests;
