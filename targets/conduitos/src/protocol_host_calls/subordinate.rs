//! Composed borrowed idle observation. Association/receipt ownership is separate.
use super::*;

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    /// Check both original native ownership and original scheduler obligations.
    /// This is not a settlement receipt, storage possession, or class validation.
    /// A preparation-owned transaction association must supply those separately.
    pub fn validate_usb_subordinate_idle(
        &self,
        kernel: &KernelCompositeHost,
    ) -> Result<(), ProtocolCallRefusal> {
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        let fragment = kernel
            .definition()
            .internal_plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == self.identity.host_id)
            .ok_or(ProtocolCallRefusal::InvalidPlan)?;
        if fragment.boot_id != self.identity.boot_id {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        if self.pending_usb.is_some()
            || self.pending_usb_failed
            || self.pending_duplex.iter().any(Option::is_some)
        {
            return Err(ProtocolCallRefusal::Unsupported);
        }
        let host = self
            .usb_duplex
            .as_ref()
            .ok_or(ProtocolCallRefusal::Unsupported)?;
        if kernel
            .has_retained_host_call_for_nodes(&self.identity.host_id, &host.subordinate_nodes())
            .map_err(ProtocolCallRefusal::Kernel)?
        {
            return Err(ProtocolCallRefusal::Unsupported);
        }
        host.validate_settlement_idle()
            .map_err(ProtocolCallRefusal::UsbProtocol)
    }
}
