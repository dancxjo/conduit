//! A finite caller with independently retained JSON residence and Body State.
use super::*;
impl StdHost {
    /// Execute one sealed fragment with this exact selected JSON provider and
    /// explicit Body-durable State residence. Selection grants no producer ACK.
    /// Current output history is admitted separately from queue capacity (1..=64
    /// values per port); an initial value plus a committed value needs two slots.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_durable_json_snapshot_to<W: Write, T: TimerAdapter>(
        &mut self,
        body: &conduit_body::BodyId,
        residence_root: &std::path::Path,
        selected: &crate::json_snapshot_file::SelectedJsonSnapshotResidence,
        fragment: PlanFragment,
        inputs: &[ExternalForeInput],
        current_output_capacity: u16,
        output_adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<StdRunReport, String> {
        validate_current_output_capacity(current_output_capacity)?;
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                json_snapshot: Some(selected),
                keyboard: None,
                indicator: None,
                retained: None,
                attach_live: false,
                external_fore: Some(ExternalForeRun {
                    current_output_capacity: Some(current_output_capacity),
                    inputs,
                    output: output_adapter,
                    sequential: false,
                    stream: None,
                    live: None,
                }),
                spoken_mask: None,
                direct_spoken_mask: None,
                durable_state: Some(installed_std::DurableStateRun {
                    body,
                    root: residence_root,
                }),
            },
        )
        .map(|run| run.report)
    }
}

#[cfg(test)]
mod tests;
