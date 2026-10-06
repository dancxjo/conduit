//! Controlled ordinary Mask execution using the shared host lifecycle.
use super::*;
impl StdHost {
    /// Execute the direct Face-to-artifact Mask on this exact Host Boot.
    #[allow(clippy::too_many_arguments)]
    pub fn run_direct_spoken_mask_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        preparation: crate::direct_spoken_mask_runtime::DirectSpokenMaskPreparation,
        inputs: &[ExternalForeInput],
        output_adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                keyboard: None,
                indicator: None,
                retained: None,
                attach_live: false,
                external_fore: Some(ExternalForeRun {
                    inputs,
                    output: output_adapter,
                    sequential: false,
                }),
                spoken_mask: None,
                direct_spoken_mask: Some(preparation),
                durable_state: None,
            },
        )
        .map(|run| run.report)
    }

    pub fn run_spoken_mask_plot_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        preparation: crate::spoken_mask_runtime::SpokenMaskPreparation,
        inputs: &[ExternalForeInput],
        output_adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
        self.run_spoken_mask_plot_controlled_to(
            fragment,
            preparation,
            inputs,
            output_adapter,
            output,
            timer,
            &RunControl::default(),
        )
    }

    /// Preserve the caller's exact stop request through ordinary Mask cleanup.
    #[allow(
        clippy::too_many_arguments,
        reason = "controlled sibling preserves the existing Mask run inputs"
    )]
    pub fn run_spoken_mask_plot_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        preparation: crate::spoken_mask_runtime::SpokenMaskPreparation,
        inputs: &[ExternalForeInput],
        output_adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                keyboard: None,
                indicator: None,
                retained: None,
                attach_live: false,
                external_fore: Some(ExternalForeRun {
                    inputs,
                    output: output_adapter,
                    sequential: false,
                }),
                spoken_mask: Some(preparation),
                direct_spoken_mask: None,
                durable_state: None,
            },
        )
        .map(|run| run.report)
    }
}
