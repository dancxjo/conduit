//! One exact input delivered by the same admitted Mask execution that rendered it.
use super::*;
use conduit_kernel::scheduler::{HostCallRequest, RemoteTerminalDisposition, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome, SignSink};
use conduit_presentation::{FaceInteraction, MaskInteractionCorrelation, MaskShow};

/// A finite session owns its pending Host Call and exact available Show. Consuming
/// operations prevent duplicate delivery; dropping a live session cancels its Play.
pub struct NativeMaskInteractionSession {
    pub(super) play: PreparedNativeMaskPlay,
    pub(super) request: HostCallRequest,
}

impl NativeMaskInteractionSession {
    pub fn show(&self) -> &MaskShow {
        &self.play.renderer.prepared_show
    }

    pub fn render_receipt(&self) -> &NativeMaskPlayReceipt {
        self.play
            .receipt
            .as_ref()
            .expect("rendered session retains its receipt")
    }

    pub fn interact(
        mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, NativeMaskPlayError> {
        interaction
            .validate_against(&self.play.renderer.presentation, self.show())
            .map_err(NativeMaskPlayError::Interaction)?;
        let bytes = interaction.encode();
        if bytes.len() > self.play.interaction_fore.byte_capacity as usize
            || bytes.len() > STORAGE_VALUE_BYTES
        {
            return Err(NativeMaskPlayError::Pressure);
        }
        let correlation = self
            .show()
            .correlate_interaction(interaction)
            .map_err(|_| NativeMaskPlayError::Presentation)?;
        let value = self
            .play
            .scheduler
            .store_host_value(&bytes)
            .map_err(|error| match error {
                conduit_kernel::scheduler::SchedulerError::Cancelled => {
                    NativeMaskPlayError::Cancelled
                }
                _ => NativeMaskPlayError::Pressure,
            })?;
        self.complete_input(Some(
            BoundedValueRef::new(value, bytes.len() as u32)
                .map_err(|_| NativeMaskPlayError::Value)?,
        ))?;
        self.finish(Some(&bytes))?;
        Ok(correlation)
    }

    /// Normal input exhaustion is distinct from cancelling the admitted Play.
    pub fn close_without_input(mut self) -> Result<NativeMaskPlayReceipt, NativeMaskPlayError> {
        self.complete_input(None)?;
        self.finish(None)?;
        let mut receipt = self
            .play
            .receipt
            .take()
            .ok_or(NativeMaskPlayError::Kernel)?;
        receipt.kernel_signs = self.play.scheduler.signs().len();
        Ok(receipt)
    }

    pub fn cancel(self) -> Result<(), NativeMaskPlayError> {
        self.play.cancel()
    }

    fn complete_input(
        &mut self,
        output: Option<BoundedValueRef>,
    ) -> Result<(), NativeMaskPlayError> {
        self.play
            .scheduler
            .complete_host_call(
                self.request.node,
                self.request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output,
                    failure: None,
                },
            )
            .map_err(|_| NativeMaskPlayError::HostComplete)
    }

    fn finish(&mut self, expected: Option<&[u8]>) -> Result<(), NativeMaskPlayError> {
        let port = &self.play.interaction_fore;
        let mut observed = false;
        for _ in 0..64 {
            if self.play.scheduler.next_host_request().is_some() {
                return Err(NativeMaskPlayError::HostComplete);
            }
            if let Some(offer) = self
                .play
                .scheduler
                .remote_egress_offer(port.endpoint, port.cord)
                .map_err(|_| NativeMaskPlayError::ForeOutput)?
            {
                if observed
                    || Some(
                        self.play
                            .scheduler
                            .host_value(offer.value)
                            .map_err(|_| NativeMaskPlayError::Value)?,
                    ) != expected
                {
                    return Err(NativeMaskPlayError::ForeOutput);
                }
                self.play
                    .scheduler
                    .remote_egress_accept(port.endpoint, port.cord, offer.sequence)
                    .and_then(|_| {
                        self.play.scheduler.remote_egress_delivered(
                            port.endpoint,
                            port.cord,
                            offer.sequence,
                        )
                    })
                    .map_err(|_| NativeMaskPlayError::ForeOutput)?;
                observed = true;
            }
            let status = self
                .play
                .scheduler
                .step()
                .map_err(|_| NativeMaskPlayError::Kernel)?;
            if matches!(status, SchedulerStatus::Drained) {
                if observed != expected.is_some() {
                    return Err(NativeMaskPlayError::ForeOutput);
                }
                for boundary in [&self.play.show_fore, &self.play.interaction_fore] {
                    if self
                        .play
                        .scheduler
                        .remote_egress_terminal_disposition(boundary.endpoint, boundary.cord)
                        .map_err(|_| NativeMaskPlayError::ForeOutput)?
                        != Some(RemoteTerminalDisposition::NormalClose)
                    {
                        return Err(NativeMaskPlayError::ForeOutput);
                    }
                }
                self.play.retired = true;
                return Ok(());
            }
            if !matches!(status, SchedulerStatus::Progress { .. }) {
                return Err(NativeMaskPlayError::Kernel);
            }
        }
        Err(NativeMaskPlayError::Kernel)
    }
}
