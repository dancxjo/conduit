//! Explicit opt-in to the existing bounded scheduler debugger observer.
use super::*;
impl StdHost {
    /// Run the ordinary Body with a prepared 128-record, 64-byte-preview
    /// debugger projection. Mandatory Signs and scheduling are unchanged.
    /// History may overwrite; its exact gap remains in the terminal report.
    /// The current observer admits canonical, one-slot Cords only; wider queues
    /// refuse preparation rather than invent FIFO causal parent identities.
    pub fn run_body_plan_with_observed_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
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
            true,
            |_, _| Ok(()),
        )
    }
}
