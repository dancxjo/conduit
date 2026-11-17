//! Capacity receipts and original IMAGE lease attachment for finite output.
use super::*;
impl BodyForeRoute<'_> {
    pub(in crate::installed_std::body_kernel) fn tick_backing_bytes(&self) -> Option<usize> {
        if !self.deliveries.is_empty()
            || self.pending_output.is_some()
            || !self.acknowledged_inputs.is_empty()
        {
            return None;
        }
        let mut bytes = self.inputs.empty_backing_bytes()?;
        for n in [
            self.ports
                .capacity()
                .checked_mul(core::mem::size_of::<LoweredForePort>())?,
            self.output_slots
                .capacity()
                .checked_mul(core::mem::size_of::<OutputSlots>())?,
            self.deliveries
                .capacity()
                .checked_mul(core::mem::size_of::<ExternalForeDelivery>())?,
            self.acknowledged_inputs
                .capacity()
                .checked_mul(core::mem::size_of::<AcknowledgedInput>())?,
        ] {
            bytes = bytes.checked_add(n)?;
        }
        for port in &self.ports {
            bytes = bytes.checked_add(tick_port_bytes(port)?)?;
        }
        for slot in &self.output_slots {
            if slot.input_origin.is_some()
                || slot.acknowledged_descriptor.is_some()
                || slot.acknowledged_physical_event.is_some()
            {
                return None;
            }
            bytes = bytes.checked_add(tick_port_bytes(&slot.port)?)?;
            bytes = bytes.checked_add(
                slot.available
                    .capacity()
                    .checked_mul(core::mem::size_of::<ExternalForeDelivery>())?,
            )?;
            // Shared immutable identities are conservatively charged per slot.
            for delivery in &slot.available {
                bytes = bytes.checked_add(delivery.retained_backing_bytes()?)?;
            }
        }
        Some(bytes)
    }
    pub(in crate::installed_std::body_kernel) fn attach_image_memory(
        &mut self,
        lease: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        for slot in &mut self.output_slots {
            for delivery in &mut slot.available {
                delivery.attach_image_memory(lease);
            }
        }
    }
}

fn tick_port_bytes(port: &LoweredForePort) -> Option<usize> {
    if port.direction != PortDirection::Output || port.selected_line.is_some() {
        return None;
    }
    let mut bytes = port
        .front_port_id
        .0
        .capacity()
        .checked_add(port.value_kind.0.capacity())?;
    bytes = bytes.checked_add(port.abnormal_kind.as_ref().map_or(0, |id| id.0.capacity()))?;
    if let Some(contract) = &port.value_contract {
        if !contract.constraints.is_empty() {
            return None;
        }
        bytes = bytes
            .checked_add(contract.value_kind.0.capacity())?
            .checked_add(
                contract
                    .constraints
                    .capacity()
                    .checked_mul(core::mem::size_of::<conduit_core::ValueConstraint>())?,
            )?;
    }
    Some(bytes)
}
