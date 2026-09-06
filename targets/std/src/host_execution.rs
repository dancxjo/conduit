//! Ordinary Host reservation, execution and owned State result handling.
use super::*;
mod reporting;
pub(crate) mod continuity;

struct HostRunInputs<'a> {
    keyboard: Option<&'a mut dyn hosted_keyboard::HostedKeyboardAdapter>,
    retained: Option<&'a mut Vec<state_value::RetainedTypedState>>,
}

impl StdHost {
    pub fn run_fragment_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_controlled_to(fragment, output, timer, &RunControl::default())
    }

    pub fn run_fragment_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_controlled_with_keyboard_to(fragment, output, timer, control, None)
    }

    pub fn run_fragment_controlled_with_keyboard_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        keyboard: Option<&mut dyn hosted_keyboard::HostedKeyboardAdapter>,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                keyboard,
                retained: None,
            },
        )
        .map(|run| run.report)
    }

    /// Retain typed State ownership after ordinary admitted execution ends.
    pub fn run_fragment_retaining_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<state_value::RetainedStdRun, String> {
        self.run_fragment_owned_with_keyboard_to(
            fragment,
            output,
            timer,
            control,
            HostRunInputs {
                keyboard: None,
                retained: None,
            },
        )
    }

    fn run_fragment_owned_with_keyboard_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        inputs: HostRunInputs<'_>,
    ) -> Result<state_value::RetainedStdRun, String> {
        let HostRunInputs { keyboard, retained } = inputs;
        write_operator_report(output, self.advertisement(), &fragment.plan_id, &fragment)?;

        let installed_standard = installed_std::supports(&fragment);
        if !installed_standard && !is_installed_kernel_signal_profile(&fragment) {
            return Err("fragment does not match the installed std kernel profile".to_string());
        }

        let advertisement = self.advertisement().clone();
        if let Some(sources) = retained.as_deref() {
            let lowered = installed_std::lower_fragment_with_continuity(&fragment, true)?;
            let identity = conduit_core::bind_active_play(
                &fragment.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                self.next_kernel_play_sequence,
            );
            installed_std::validate_retained_inputs(&fragment, &lowered, &identity, sources)?;
        }
        let reservation = self.kernel_resources.prepare_and_reserve_with_continuity(
            &advertisement,
            &fragment,
            retained.is_some(),
        )?;
        let result = (|| {
            let play = self.issue_kernel_play(&fragment)?;
            let play_sequence = play.identity.play_sequence;
            if installed_standard {
                installed_std::run_fragment_retaining(
                    installed_std::InstalledRunHost {
                        advertisement: &advertisement,
                        playback: self.playback.as_ref(),
                        midi_input: self.midi_input.as_ref(),
                        midi_output: self.midi_output.as_ref(),
                        keyboard,
                        local_model: self.local_model.as_deref_mut(),
                        vector_search: self.vector_search.as_deref_mut(),
                        calendar: self.calendar.as_deref_mut(),
                    },
                    &fragment,
                    play_sequence,
                    &mut self.next_kernel_sign_sequence,
                    output,
                    timer,
                    installed_std::RunLifecycle { control, retained },
                )
            } else {
                if control.requested_stop().is_some() {
                    return Err("kernel-signal profile cannot accept generic Run control".into());
                }
                kernel_signal::run_signal_fragment(
                    &advertisement,
                    &fragment,
                    play_sequence,
                    &mut self.next_kernel_sign_sequence,
                    output,
                    timer,
                )
                .map(|report| state_value::RetainedStdRun {
                    report,
                    states: Vec::new(),
                })
            }
        })();
        let release = self.kernel_resources.release(reservation);
        let run = result?;
        release?;
        let report = &run.report;
        reporting::write_terminal(output, &fragment, report)?;
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
        Ok(run)
    }
}
