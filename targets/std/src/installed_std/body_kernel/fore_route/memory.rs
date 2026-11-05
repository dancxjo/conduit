//! Owned local Fore metadata and sealed output slots. Counting supplies no grant
//! or complete Play receipt, and says nothing about validation workspace.
use super::*;
impl BodyForeRoute<'_> {
    /// Existing closed output-only profile retains its execution restrictions.
    pub(in crate::installed_std::body_kernel) fn tick_backing_bytes(&self) -> Option<usize> {
        self.inputs.empty_backing_bytes()?;
        if self
            .ports
            .iter()
            .chain(self.output_slots.iter().map(|slot| &slot.port))
            .any(|port| {
                port.direction != PortDirection::Output
                    || port.value_contract.as_ref().is_some_and(|c| {
                        !c.constraints.is_empty() || c.value_kind != port.value_kind
                    })
            })
        {
            return None;
        }
        self.local_one_shot_backing_bytes()
    }
    /// Closed finite one-shot profile: borrowed ingress, sealed output slots,
    /// and only nonallocating primitive validation with no dynamic constraints.
    /// Opaque Info handling is unchanged; Native laws and authority remain the
    /// independently selected provider/owner's obligation, not this receipt.
    pub(in crate::installed_std::body_kernel) fn finite_one_shot_backing_bytes(
        &self,
    ) -> Option<usize> {
        use conduit_core::PrimitiveInfoKind as P;
        for port in self
            .ports
            .iter()
            .chain(self.output_slots.iter().map(|slot| &slot.port))
        {
            if port.quantity_profile != conduit_core::QuantityValueProfile::Decimal
                || port
                    .value_contract
                    .as_ref()
                    .is_some_and(|c| !c.constraints.is_empty() || c.value_kind != port.value_kind)
                || conduit_core::parse_quantity_role_info_id(port.value_kind.as_str()).is_some()
                || conduit_core::quantity_info_dimension(port.value_kind.as_str()).is_some()
                || port.value_kind.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID
                || !matches!(
                    conduit_core::primitive_info_kind(port.value_kind.as_str()),
                    None | Some(
                        P::Empty
                            | P::CancellationRequest
                            | P::Bool
                            | P::Count
                            | P::Bytes
                            | P::Text
                            | P::F32
                            | P::F64
                    )
                )
            {
                return None;
            }
        }
        self.local_one_shot_backing_bytes()
    }
    /// Account actual one-shot ingress/output metadata, including nested value
    /// constraints. Input payload bytes retain their separate caller owner.
    pub(in crate::installed_std::body_kernel) fn local_one_shot_backing_bytes(
        &self,
    ) -> Option<usize> {
        if !self.deliveries.is_empty()
            || self.pending_output.is_some()
            || !self.acknowledged_inputs.is_empty()
        {
            return None;
        }
        let mut bytes = self.inputs.one_shot_backing_bytes()?;
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
            bytes = bytes.checked_add(local_port_bytes(port)?)?;
        }
        for slot in &self.output_slots {
            if slot.input_origin.is_some()
                || slot.acknowledged_descriptor.is_some()
                || slot.acknowledged_physical_event.is_some()
            {
                return None;
            }
            bytes = bytes.checked_add(local_port_bytes(&slot.port)?)?;
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
}
fn local_port_bytes(port: &LoweredForePort) -> Option<usize> {
    if port.selected_line.is_some() {
        return None;
    }
    let mut bytes = port
        .front_port_id
        .0
        .capacity()
        .checked_add(port.value_kind.0.capacity())?;
    bytes = bytes.checked_add(port.abnormal_kind.as_ref().map_or(0, |id| id.0.capacity()))?;
    if let Some(contract) = &port.value_contract {
        bytes = bytes.checked_add(contract.retained_backing_bytes()?)?;
    }
    Some(bytes)
}
#[cfg(test)]
#[path = "memory/tests.rs"]
mod tests;
