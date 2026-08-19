//! External platform manifestation seam for the installed std Signal kernel.

use std::io::Write;

use conduit_core::PlanFragment;

use crate::{
    kernel_signal, write_operator_report, RunControl, StdHost, StdRunReport, TimerAdapter,
};

pub use crate::kernel_signal::SignalManifestation;

impl StdHost {
    /// Run an exact installed Signal fragment while delegating only the
    /// `presentation/show` platform effect to an admitted external adapter.
    pub fn run_signal_fragment_with_manifestation<W, T, M>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        manifestation: &mut M,
    ) -> Result<StdRunReport, String>
    where
        W: Write,
        T: TimerAdapter,
        M: SignalManifestation,
    {
        if control.requested_stop().is_some() {
            return Err("kernel-signal profile cannot accept generic Run control".into());
        }
        write_operator_report(output, self.advertisement(), &fragment.plan_id, &fragment)?;
        if !crate::is_installed_kernel_signal_profile(&fragment) {
            return Err("fragment does not match the installed std signal kernel profile".into());
        }
        let advertisement = self.advertisement().clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, &fragment)?;
        let play_sequence = self.next_kernel_play_sequence;
        self.next_kernel_play_sequence = play_sequence
            .checked_add(1)
            .ok_or_else(|| "kernel Play sequence exhausted".to_string())?;
        let result = kernel_signal::run_signal_fragment_with_manifestation(
            &advertisement,
            &fragment,
            play_sequence,
            &mut self.next_kernel_sign_sequence,
            output,
            timer,
            manifestation,
        );
        let release = self.kernel_resources.release(reservation);
        let report = result?;
        release?;
        writeln!(output, "plan {} complete", fragment.plan_id.as_str())
            .map_err(|error| error.to_string())?;
        if let (Some(first), Some(last)) = (report.receipts.first(), report.receipts.last()) {
            writeln!(
                output,
                "receipts {} first=({}, {}) last=({}, {})",
                report.receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "receipts 0").map_err(|error| error.to_string())?;
        }
        Ok(report)
    }
}
