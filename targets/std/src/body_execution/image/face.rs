//! Selected chart execution retains the original admitted worst-case escaped
//! contribution charge. Observer backing/effects remain a separate obligation.
use super::*;
impl StdHost {
    #[allow(clippy::too_many_arguments)]
    pub fn run_selected_image_body_plan_with_face_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore: &mut dyn BodyForeOutputAdapter,
        face: &mut dyn BodyFaceContributionObserver,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if !self.kernel_resources.has_image_bounds() {
            return Err("selected Face execution requires an IMAGE-bound Host".into());
        }
        let mut custody = None;
        let report = self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            Some((inputs, sequential, fore)),
            None,
            None,
            None,
            None,
            started,
            Some(&mut custody),
            Some(face),
            None,
        )?;
        Ok(ImageBodyRunReport {
            report,
            _memory: custody.ok_or("selected Face result lost its memory custody")?,
        })
    }
}
