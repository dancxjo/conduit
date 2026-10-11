//! Read-only validation of original native obligations; no stop or revocation.
use super::*;

impl UsbNativeHost<'_> {
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
