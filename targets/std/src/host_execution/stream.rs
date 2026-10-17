//! Explicit finite overlapping ingress through the ordinary installed kernel.
use super::*;

/// Host-side admission work per kernel turn, independent of sealed Cord capacity.
/// Every payload still passes exact contract validation and atomic fan-out admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalForeStreamConfig {
    pub admission_batch: u16,
}
impl ExternalForeStreamConfig {
    pub const MAX_INPUTS: u16 = 4096;
    pub const MAX_BATCH: u16 = 64;
    pub(crate) fn validate(self) -> Result<(), String> {
        if self.admission_batch == 0 || self.admission_batch > Self::MAX_BATCH {
            return Err("external Fore stream admission batch must be 1 to 64".into());
        }
        Ok(())
    }
}

/// Actual ingress observations. Full attempts consume no input or sequence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ExternalForeStreamStats {
    pub admission_batch: u16,
    pub batches: u32,
    pub attempts: u32,
    pub accepted: u16,
    pub full: u32,
}

impl StdHost {
    /// Offer a prevalidated finite stream while earlier inputs are still executing.
    /// Unlike causal sequences, this allows overlapping values. Queue capacity,
    /// pressure, ordering, mandatory Signs and cancellation remain kernel-owned.
    /// Input lifetime and all staging/history storage are admitted before Play.
    #[allow(clippy::too_many_arguments)]
    pub fn run_external_plot_stream_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        inputs: &[ExternalForeInput],
        output_adapter: &mut dyn ExternalForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        config: ExternalForeStreamConfig,
    ) -> Result<StdRunReport, String> {
        config.validate()?;
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
                    sequential: true,
                    stream: Some(config),
                }),
                spoken_mask: None,
                direct_spoken_mask: None,
                durable_state: None,
            },
        )
        .map(|run| run.report)
    }
}
