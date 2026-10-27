//! Typed finite and live Fore entrances share the ordinary Body runner.
use super::*;

impl StdHost {
    /// Execute a sealed local Body with caller-supplied, typed Fore values and
    /// an acknowledging output adapter. The exact scoped, finite preloaded
    /// Todo scan is admitted; later Mask actions and other activation routes
    /// remain unavailable.
    pub fn run_body_plan_with_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_with_fore_to_with_start(
            request,
            inputs,
            sequential,
            fore_output,
            output,
            timer,
            |_, _| Ok(()),
        )
    }

    /// Run one retained Body Play while an admitted typed queue receives
    /// later values. Face/Mask action routing is a separate semantic boundary;
    /// the queue must be closed explicitly to complete.
    pub fn run_body_plan_with_live_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_with_live_fore_to_with_start(
            request,
            queue,
            fore_output,
            output,
            timer,
            |_, _| Ok(()),
        )
    }

    /// Publish the admitted live Play before waiting for later typed actions.
    pub fn run_body_plan_with_live_fore_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((queue, fore_output)),
            None,
            None,
            started,
        )
    }

    /// Publish the exact admitted Play before the kernel advances. A caller may
    /// durably retain the start and refuse execution if that publication fails.
    /// The callback is never invoked for preparation or reservation refusal.

    /// Report the genuine admitted start before finite Fore work reaches effects.
    /// This lets a Body lifecycle owner correlate both execution and publication.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_with_fore_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            Some((inputs, sequential, fore_output)),
            None,
            None,
            None,
            started,
        )
    }
}
