//! Selected IMAGE result custody. Escaped immutable Fore payload clones retain
//! the shared byte charge; borrowing cannot move other report backing.
use super::*;
use crate::kernel_preparation::ImageMemoryLease;
mod prepared;
pub use prepared::{
    ImageMemoryRetention, ImagePreparedValue, ImageRootPreparation, ImageRootPreparationOwner,
};
mod creche;
mod face;
mod input;
mod live;
pub use input::ImageForeInput;
pub use live::ImageLiveForeQueue;

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
    /// Cooperative execution within the qualified local memory profile. Its
    /// byte charge remains until this wrapper and escaped payload clones drop; timer slots
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
        self.run_selected_image_body_plan_with_fore_to_with_start(
            request,
            &[],
            output,
            timer,
            fore,
            started,
        )
    }
    /// Execute one explicitly selected, already retained finite input without
    /// copying its payload or detaching its original IMAGE memory charge.
    /// The caller still owns Source authority, exact planned Fore selection,
    /// output custody and preparation; shared backing alone grants none of them.
    #[allow(clippy::too_many_arguments)] // Preserve the ordinary execution boundary.
    pub fn run_selected_image_body_plan_with_retained_input_to_with_start<
        W: Write,
        T: TimerAdapter,
        F,
    >(
        &mut self,
        request: BodyRunRequest<'_>,
        input: &ImageForeInput,
        output: &mut W,
        timer: &mut T,
        fore: &mut dyn BodyForeOutputAdapter,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.verify_selected_image_fore_input(input)?;
        self.run_selected_image_body_plan_with_fore_to_with_start(
            request,
            std::slice::from_ref(input.input()),
            output,
            timer,
            Some(fore),
            started,
        )
    }

    /// Finite local ingress borrows caller-owned immutable bytes for this call.
    /// Their storage, output adapter and preparation effects retain independent
    /// custody. Streaming, Lines and unqualified providers remain refused.
    pub fn run_selected_image_body_plan_with_fore_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        output: &mut W,
        timer: &mut T,
        fore: Option<&mut dyn BodyForeOutputAdapter>,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if !inputs.is_empty() && fore.is_none() {
            return Err("selected IMAGE finite ingress requires an output adapter".into());
        }
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
            fore.map(|adapter| (inputs, false, adapter)),
            None,
            None,
            None,
            None,
            started,
            Some(&mut custody),
            None,
            None,
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
