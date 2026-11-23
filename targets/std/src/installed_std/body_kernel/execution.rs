//! Execution of the existing admitted Body kernel.
use super::*;
mod drive;
mod finish;
pub(crate) mod state;
impl BodyKernel<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run<'observer, W: Write, T: TimerAdapter>(
        mut self,
        output: &mut W,
        clock: &mut T,
        input: Option<&mut dyn HostedKeyboardAdapter>,
        mut fore_output: Option<&mut dyn BodyForeOutputAdapter>,
        control: &RunControl,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        body_time: Option<(&BodyTimeRequirement, &BodyClockCorrelation)>,
        execution_bounds: Option<(
            conduit_core::MonotonicDuration,
            conduit_core::MonotonicDuration,
        )>,
        admitted_clock_quality: Option<BodyTimeQuality>,
        mut terminal: Option<crate::body_execution::ParsecTerminalAttachment<'_>>,
        mut creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
        face_observer: Option<
            &'observer mut dyn crate::body_execution::face_contribution::BodyFaceContributionObserver,
        >,
    ) -> BodyKernelResult {
        let mut state = self.execution_state(input, admitted_clock_quality);
        state.face_observer = face_observer;
        let result = loop {
            match self.drive(
                &mut state,
                output,
                clock,
                &mut fore_output,
                control,
                host_id,
                boot_id,
                body_time,
                execution_bounds,
                &mut terminal,
                &mut creche,
                true,
                false,
                &|| false,
            ) {
                Ok(Some(terminal)) => break Ok(terminal),
                Ok(None) => {}
                Err(error) => break Err(error),
            }
        };
        self.finish(
            state,
            result,
            clock,
            host_id,
            boot_id,
            execution_bounds,
            terminal,
            creche,
        )
    }
}
