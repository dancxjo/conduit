//! Explicit Body/residence custody combined with the existing finite Fore runner.
use super::*;

impl StdHost {
    /// Execute a fragment whose Plan selected the Body-durable State Back.
    /// The Body identity and host-selected residence enter explicitly here;
    /// neither is inferred from the Plot, Plan id, or filesystem path.
    pub fn run_body_durable_fragment_to<W: Write, T: TimerAdapter>(
        &mut self,
        body: &conduit_body::BodyId,
        residence_root: &std::path::Path,
        fragment: PlanFragment,
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
                external_fore: None,
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

    /// Execute finite Plan-sealed Fore inputs with an explicitly supplied Body
    /// and existing host-selected durable State residence. Caller-owned identity
    /// and residence must be independently admitted; this method discovers no
    /// Base and issues no grants. Fore contract checks, finite input preparation,
    /// cancellation and State commit/recovery use the existing installed kernel.
    /// This entry accepts one value per exact input boundary; it enables neither
    /// live ingress nor sequential/overlapping streams or recorder ACK policy.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_durable_external_plot_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        body: &conduit_body::BodyId,
        residence_root: &std::path::Path,
        fragment: PlanFragment,
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
