//! Existing durable State and sealed external Fore in one ordinary execution.
use super::*;
impl StdHost {
    /// Root supplies actual Body/residence custody and the exact external Fore.
    /// Neither input delivery nor an output adapter creates storage authority.
    /// All execution, State synchronization and admission stay on the existing
    /// installed-kernel path; no separate persistence or scheduler is introduced.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_durable_external_plot_to<'a, W: Write, T: TimerAdapter>(
        &mut self,
        body: &'a conduit_body::BodyId,
        residence_root: &'a std::path::Path,
        fragment: PlanFragment,
        external_fore: ExternalForeRun<'a>,
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
                external_fore: Some(external_fore),
                spoken_mask: None,
                direct_spoken_mask: None,
                #[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
                snapshot_directory: None,
                durable_state: Some(installed_std::DurableStateRun {
                    body,
                    root: residence_root,
                }),
            },
        )
        .map(|run| run.report)
    }
}

#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
impl StdHost {
    /// Root independently supplies directory custody and the Body State residence.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_durable_snapshot_plot_to<'a, W: Write, T: TimerAdapter>(
        &mut self,
        body: &'a conduit_body::BodyId,
        residence_root: &'a std::path::Path,
        snapshots: &'a conduit_linux_snapshot_store::SnapshotDirectory,
        fragment: PlanFragment,
        external_fore: ExternalForeRun<'a>,
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
                external_fore: Some(external_fore),
                spoken_mask: None,
                direct_spoken_mask: None,
                snapshot_directory: Some(snapshots),
                durable_state: Some(installed_std::DurableStateRun {
                    body,
                    root: residence_root,
                }),
            },
        )
        .map(|run| run.report)
    }
}
