//! Class-neutral commands for a retained, quiesced endpoint queue.
//! These commands confer neither operation authority nor software retirement.
#![allow(dead_code)] // Recovery composition is not installed yet.
use super::{XhciError, XhciReady};

impl XhciReady {
    fn endpoint_command_exact(
        &mut self,
        slot: u8,
        endpoint: u8,
        command_type: u32,
        parameter: u64,
    ) -> Result<(), XhciError> {
        if slot == 0 || slot > self.admitted_slots || !(2..=31).contains(&endpoint) {
            return Err(XhciError::InvalidLayout);
        }
        let event = self.command([
            parameter as u32,
            (parameter >> 32) as u32,
            0,
            (command_type << 10) | (u32::from(endpoint) << 16) | (u32::from(slot) << 24),
        ])?;
        if event.slot != slot || event.completion_code != 1 {
            return Err(XhciError::UnexpectedCompletion);
        }
        Ok(())
    }

    /// # Safety
    /// Caller exclusively owns this original endpoint and its event consumer,
    /// has observed its exact halted state, and has closed further submissions.
    /// All original DMA and obligations remain retained until separately settled.
    /// A successful reset does not authorize replay or a new submission.
    pub(crate) unsafe fn reset_original_halted_endpoint(
        &mut self,
        slot: u8,
        endpoint: u8,
    ) -> Result<(), XhciError> {
        self.endpoint_command_exact(slot, endpoint, 14, 0)
    }

    /// # Safety
    /// Original controller has acknowledged this endpoint's stopped state; every
    /// original TD/event and software/Kernel obligation has separately retired.
    /// Caller exclusively retains the exact coherent ring and its producer
    /// history. This address/cycle is its next unused position, not a replay TD.
    /// No new TD may be published or doorbell rung until this exact command
    /// succeeds. Failure retains quarantine and all DMA, including after a
    /// preceding successful Reset Endpoint command.
    pub(crate) unsafe fn set_original_stopped_endpoint_dequeue(
        &mut self,
        slot: u8,
        endpoint: u8,
        address: u64,
        cycle: u32,
    ) -> Result<(), XhciError> {
        if address == 0 || address & 15 != 0 || cycle > 1 {
            return Err(XhciError::InvalidLayout);
        }
        self.endpoint_command_exact(slot, endpoint, 16, address | u64::from(cycle))
    }
}
