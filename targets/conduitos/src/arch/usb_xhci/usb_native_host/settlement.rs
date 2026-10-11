//! Read-only validation of original native obligations; no stop or revocation.
use super::*;

impl UsbNativeHost<'_> {
    fn validate_cancelled_transfers(&self) -> Result<(), UsbNativeHostRefusal> {
        if !self.stop_acknowledged
            || !self.stopped
            || !self.possessions_revoked
            || self.uncertain_control
            || self.uncertain_bulk
            || self.pending_endpoint.is_some()
            || self.pending_write.is_some()
            || self.isochronous.is_some()
            || self.calls.iter().any(|bound| match &bound.transfer {
                Transfer::Control(transfer) => transfer.retains_settlement_obligation(),
                Transfer::Endpoint(transfer) => transfer.retains_settlement_obligation(),
                Transfer::Write(transfer) => transfer.retains_settlement_obligation(),
            })
        {
            return Err(UsbNativeHostRefusal::Pending);
        }
        Ok(())
    }

    pub(super) fn discard_cancelled_terminals(&mut self) -> Result<(), UsbNativeHostRefusal> {
        self.validate_cancelled_transfers()?;
        // Caller has cancelled/retired every original kernel obligation before
        // discarding unpublished software outputs. Native stop alone is not enough.
        if let Some(slots) = &mut self.bulk_terminals {
            for slot in slots.iter_mut() {
                slot.index = None;
                slot.length = 0;
            }
        }
        self.bulk_terminals_ready = false;
        Ok(())
    }

    pub(super) fn validate_cancelled_settlement(&self) -> Result<(), UsbNativeHostRefusal> {
        self.validate_cancelled_transfers()?;
        if self
            .bulk_terminals
            .as_ref()
            .is_some_and(|slots| slots.iter().any(|slot| slot.index.is_some()))
        {
            return Err(UsbNativeHostRefusal::Pending);
        }
        Ok(())
    }

    /// This certifies only present native idle, never class protocol correctness.
    pub(super) fn validate_settlement_idle(&self) -> Result<(), UsbNativeHostRefusal> {
        self.idle_transport()?;
        if self.possessions_revoked || self.isochronous.is_some() {
            return Err(UsbNativeHostRefusal::NotReady);
        }
        super::super::attachment::validate_attachment_continuity(
            self.controller.port_status(self.device.root_port),
        )
        .map_err(UsbNativeHostRefusal::Native)?;
        for bound in &self.calls {
            match &bound.transfer {
                Transfer::Control(transfer) => transfer
                    .validate_settlement_idle(&self.device)
                    .map_err(UsbNativeHostRefusal::Control)?,
                Transfer::Endpoint(transfer) => transfer
                    .validate_settlement_idle(&self.device)
                    .map_err(UsbNativeHostRefusal::Endpoint)?,
                Transfer::Write(transfer) => transfer
                    .validate_settlement_idle(&self.device)
                    .map_err(UsbNativeHostRefusal::Write)?,
            }
        }
        Ok(())
    }
}
