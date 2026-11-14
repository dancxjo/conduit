//! Additive ordinary Body execution with a finite committed semantic Face route.
//! Receiving observers must fund their own retained slot(s), preserve pressure,
//! and recheck current owner scope. This is not a Mask Show or IMAGE custody.
use super::*;
impl StdHost {
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_with_fore_and_face_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        face: &mut dyn BodyFaceContributionObserver,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            Some((inputs, sequential, fore_output)),
            None,
            None,
            None,
            None,
            started,
            None,
            Some(face),
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_with_live_fore_and_face_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        face: &mut dyn BodyFaceContributionObserver,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((queue, fore_output)),
            None,
            None,
            None,
            started,
            None,
            Some(face),
            None,
        )
    }
}
