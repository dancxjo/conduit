//! Product installation joins two admitted transports under one native owner.
#![allow(dead_code)] // Product aggregate Root admission follows this substrate.
use super::{
    UsbControlCallRefusal, UsbControlSelection, UsbDevice,
    control_installation::{ReadyUsbControlBase, validate_selection},
    endpoint_read::{EndpointNativeRefusal, EndpointReceiveDma},
    endpoint_setup::ConfiguredInboundEndpoint,
    native_host::{UsbNativeCallIndex, UsbNativeHost, UsbNativeHostRefusal},
};
use crate::{
    arch::x86_64::xhci::XhciReady,
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        control_contract::ControlContract,
        control_owner::ControlOwnerRefusal,
        endpoint_read_contract::EndpointReadContract,
        endpoint_read_factory::*,
        endpoint_read_owner::{EndpointReadAttachment, EndpointReadOwnerRefusal},
        installation::{UsbInstallationRefusal, UsbNativeIdentity},
    },
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[derive(Debug)]
pub enum UsbNativeProtocolRefusal {
    Installation(UsbInstallationRefusal),
    EndpointOffer(&'static str),
    Canonical(StructuredInfoRefusal),
    Control(UsbControlCallRefusal),
    Endpoint(EndpointNativeRefusal),
    Write(super::endpoint_write_transfer::EndpointWriteNativeRefusal),
    Host(UsbNativeHostRefusal),
}

pub(crate) struct ReadyUsbProtocolHost {
    control: ReadyUsbControlBase,
    endpoint_identity: UsbNativeIdentity,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReceiveDma<'static>,
    attachment: EndpointReadAttachment,
}
impl ReadyUsbProtocolHost {
    /// # Safety
    /// Root exclusively owns this exact native controller/device and both named
    /// providers/resources/generations/envelopes. The actual configured receipt,
    /// coherent endpoint DMA and control mapping remain rooted through every
    /// failure until acknowledged physical stop. No other event consumer runs.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(
        control_identity: UsbNativeIdentity,
        endpoint_identity: UsbNativeIdentity,
        controller: XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
        configured: ConfiguredInboundEndpoint,
        dma: EndpointReceiveDma<'static>,
        attachment: EndpointReadAttachment,
    ) -> Result<Self, UsbNativeProtocolRefusal> {
        endpoint_identity
            .validate()
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        if control_identity.host_id != endpoint_identity.host_id
            || control_identity.boot_id != endpoint_identity.boot_id
            || control_identity.artifact_id != endpoint_identity.artifact_id
            || control_identity.base_id == endpoint_identity.base_id
            || control_identity.provider_instance_id == endpoint_identity.provider_instance_id
            || control_identity.resource_pool_id == endpoint_identity.resource_pool_id
            || endpoint_identity.provider_generation != u64::from(device.attachment_epoch)
            || attachment.slot != device.slot
            || attachment.generation != u64::from(device.attachment_epoch)
            || attachment.endpoint_generation != configured.endpoint_epoch
            || attachment.endpoint_dci != configured.dci
            || attachment.resource_bytes < 3072
        {
            return Err(UsbNativeProtocolRefusal::Installation(
                UsbInstallationRefusal::InvalidIdentity,
            ));
        }
        let ring_virtual = dma.ring.as_ptr() as u64;
        let buffer_virtual = dma.buffer.as_ptr() as u64;
        let ring_end = dma.ring_physical.checked_add(1024);
        let buffer_end = dma.buffer_physical.checked_add(2048);
        if dma.ring_physical & 63 != 0
            || (dma.ring_physical & 0xffff) > 0xfc00
            || (dma.buffer_physical & 0xffff) > 0xf800
            || ring_end.is_none()
            || buffer_end.is_none()
            || dma.ring_physical < buffer_end.unwrap_or(0)
                && dma.buffer_physical < ring_end.unwrap_or(0)
            || !mapped_region(ring_virtual, dma.ring_physical, 1024, mapping)
            || !mapped_region(buffer_virtual, dma.buffer_physical, 2048, mapping)
        {
            return Err(UsbNativeProtocolRefusal::Endpoint(
                EndpointNativeRefusal::Mapping,
            ));
        }
        let contract =
            EndpointReadContract::prepare().map_err(UsbNativeProtocolRefusal::Canonical)?;
        super::endpoint_offer::ready_entry_for_identity(
            &endpoint_identity,
            &device,
            &configured,
            &dma,
            &contract,
        )
        .map_err(UsbNativeProtocolRefusal::EndpointOffer)?;
        // SAFETY: Root supplied original ownership of this exact native attachment.
        let control =
            unsafe { ReadyUsbControlBase::new(control_identity, controller, device, mapping) }
                .map_err(UsbNativeProtocolRefusal::Installation)?;
        Ok(Self {
            control,
            endpoint_identity,
            configured,
            dma,
            attachment,
        })
    }

    pub(crate) fn control_identity(&self) -> &UsbNativeIdentity {
        self.control.identity()
    }
    pub(crate) fn endpoint_identity(&self) -> &UsbNativeIdentity {
        &self.endpoint_identity
    }
    pub(crate) fn attachment_epoch(&self) -> u32 {
        self.control.attachment_epoch()
    }
    pub(crate) fn control_resource_bytes(&self) -> u64 {
        self.control.resource_bytes()
    }
    pub(crate) fn endpoint_resource_bytes(&self) -> u64 {
        self.attachment.resource_bytes
    }

    /// Project both real ready providers atomically, without grants or possession.
    pub(crate) fn append_to_advertisement(
        &self,
        host: &mut HostAdvertisement,
        control_contract: &ControlContract,
        endpoint_contract: &EndpointReadContract,
    ) -> Result<(), UsbNativeProtocolRefusal> {
        let entry = super::endpoint_offer::ready_entry_for_identity(
            &self.endpoint_identity,
            self.control.device(),
            &self.configured,
            &self.dma,
            endpoint_contract,
        )
        .map_err(UsbNativeProtocolRefusal::EndpointOffer)?;
        if host.bases.iter().any(|base| {
            base.base_id == entry.base_id || base.provider_instance_id == entry.provider_instance_id
        }) || host
            .resources
            .iter()
            .any(|resource| resource.pool_id == self.endpoint_identity.resource_pool_id)
            || host
                .capabilities
                .iter()
                .any(|offer| offer.capability_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
        {
            return Err(UsbNativeProtocolRefusal::Installation(
                UsbInstallationRefusal::Advertisement,
            ));
        }
        let mut projected = host.clone();
        self.control
            .append_to_advertisement(&mut projected, control_contract)
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                projected.capabilities.len().saturating_add(1),
            )
            .map_err(|_| {
                UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::Advertisement)
            })?,
            maximum_advertised_resources: u16::try_from(
                projected.resources.len().saturating_add(1),
            )
            .map_err(|_| {
                UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::Advertisement)
            })?,
        })
        .map_err(|_| {
            UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::Advertisement)
        })?;
        registry
            .register(entry)
            .and_then(|()| registry.project_ready_into(&mut projected))
            .map_err(|_| {
                UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::Advertisement)
            })?;
        *host = projected;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_selected(
        self,
        control_table: BaseCapabilityTable,
        control_handle: BaseCapabilityHandle,
        control_claim: BaseOperationClaim,
        control: UsbControlSelection<'_>,
        endpoint_table: BaseCapabilityTable,
        endpoint_handle: BaseCapabilityHandle,
        endpoint_claim: BaseOperationClaim,
        endpoint_contract: &EndpointReadContract,
        endpoint: SelectedOperationPlan<'_>,
    ) -> Result<NativeUsbProtocolHost, UsbNativeProtocolRefusal> {
        validate_selection(self.control.identity(), &control_claim, &control)
            .map_err(UsbNativeProtocolRefusal::Control)?;
        validate_endpoint_selection(&self.endpoint_identity, &endpoint_claim, &endpoint)?;
        let control_node = control
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == control.placement)
            .ok_or(UsbNativeProtocolRefusal::Control(
                UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding),
            ))?
            .node;
        let endpoint_node = endpoint
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == endpoint.placement_id)
            .ok_or(UsbNativeProtocolRefusal::Endpoint(
                EndpointNativeRefusal::Ownership(EndpointReadOwnerRefusal::WrongBinding),
            ))?
            .node;
        let (controller, device, mapping) = self.control.into_native_parts();
        // SAFETY: readiness retained original ownership; exact native identities
        // are preserved by both selections, independently of Root-issued grants.
        let mut host = unsafe { UsbNativeHost::new(controller, device, mapping) }
            .map_err(UsbNativeProtocolRefusal::Host)?;
        let control_index =
            unsafe { host.add_control(control_table, control_handle, control_claim, control) }
                .map_err(native_refusal)?;
        let endpoint_index = unsafe {
            host.add_endpoint(
                self.configured,
                self.dma,
                self.attachment,
                endpoint_table,
                endpoint_handle,
                endpoint_claim,
                endpoint_contract,
                endpoint,
            )
        }
        .map_err(native_refusal)?;
        Ok(NativeUsbProtocolHost {
            host,
            control_index,
            endpoint_index,
            control_node,
            endpoint_node,
        })
    }
}

pub(super) fn validate_endpoint_selection(
    identity: &UsbNativeIdentity,
    claim: &BaseOperationClaim,
    selection: &SelectedOperationPlan<'_>,
) -> Result<(), UsbNativeProtocolRefusal> {
    let refusal = || {
        UsbNativeProtocolRefusal::Endpoint(EndpointNativeRefusal::Ownership(
            EndpointReadOwnerRefusal::WrongBinding,
        ))
    };
    let gear = selection
        .fragment
        .placements
        .iter()
        .find(|gear| &gear.placement_id == selection.placement_id)
        .ok_or_else(refusal)?;
    let base = gear.base.as_ref().ok_or_else(refusal)?;
    if claim.host_id != identity.host_id
        || claim.boot_id != identity.boot_id
        || claim.base_instance_id != identity.provider_instance_id
        || claim.base_provider_generation != identity.provider_generation
        || claim.resource_pool_id != identity.resource_pool_id
        || claim.resource_generation_id != identity.resource_generation_id
        || claim.envelope_id != identity.envelope_id
        || claim.implementation_id.as_str() != ENDPOINT_READ_IMPLEMENTATION
        || gear.artifact_id != identity.artifact_id
        || base.base_id != identity.base_id
        || base.provider_instance_id != identity.provider_instance_id
        || base.provider_generation != identity.provider_generation
        || base.implementation_id.as_str() != ENDPOINT_READ_BASE
        || base.mechanism_family.as_str() != ENDPOINT_READ_ATTACHMENT
        || base.enforcement_class != BaseEnforcementClass::Cooperative
    {
        return Err(refusal());
    }
    Ok(())
}

pub(crate) struct NativeUsbProtocolHost {
    host: UsbNativeHost<'static>,
    control_index: UsbNativeCallIndex,
    endpoint_index: UsbNativeCallIndex,
    control_node: NodeId,
    endpoint_node: NodeId,
}
impl NativeUsbProtocolHost {
    pub(crate) fn control_invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbNativeProtocolRefusal> {
        if node != self.control_node || call != HostCallId(0) {
            return Err(UsbNativeProtocolRefusal::Control(
                UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding),
            ));
        }
        self.host
            .control_invoke(self.control_index, request, input)
            .map_err(native_refusal)
    }
    pub(crate) fn endpoint_begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbNativeProtocolRefusal> {
        if node != self.endpoint_node || call != HostCallId(0) {
            return Err(UsbNativeProtocolRefusal::Endpoint(
                EndpointNativeRefusal::Ownership(EndpointReadOwnerRefusal::WrongBinding),
            ));
        }
        self.host
            .begin_endpoint(self.endpoint_index, request, input)
            .map_err(native_refusal)
    }
    pub(crate) fn endpoint_poll(&mut self) -> Result<Option<&[u8]>, UsbNativeProtocolRefusal> {
        self.host
            .poll_endpoint(self.endpoint_index)
            .map_err(native_refusal)
    }
    pub(crate) fn cancel(&mut self) -> Result<(), UsbNativeProtocolRefusal> {
        self.host.cancel().map_err(native_refusal)
    }
}
pub(super) fn native_refusal(error: UsbNativeHostRefusal) -> UsbNativeProtocolRefusal {
    match error {
        UsbNativeHostRefusal::Control(error) => UsbNativeProtocolRefusal::Control(error),
        UsbNativeHostRefusal::Endpoint(error) => UsbNativeProtocolRefusal::Endpoint(error),
        UsbNativeHostRefusal::Write(error) => UsbNativeProtocolRefusal::Write(error),
        error => UsbNativeProtocolRefusal::Host(error),
    }
}

pub(super) fn mapped_region(
    start: u64,
    physical: u64,
    bytes: u64,
    mapping: fn(u64) -> Option<u64>,
) -> bool {
    let Some(last) = start.checked_add(bytes - 1) else {
        return false;
    };
    let Some(physical_last) = physical.checked_add(bytes - 1) else {
        return false;
    };
    physical != 0
        && mapping(start) == Some(physical)
        && mapping(last) == Some(physical_last)
        && (0..bytes)
            .step_by(4096)
            .all(|offset| mapping(start + offset) == Some(physical + offset))
}

#[cfg(test)]
mod tests {
    use super::mapped_region;
    #[test]
    fn actual_endpoint_dma_requires_contiguous_exact_mapping() {
        fn coherent(address: u64) -> Option<u64> {
            address.checked_add(0x10000)
        }
        fn missing_end(address: u64) -> Option<u64> {
            (address < 0x2000).then_some(address + 0x10000)
        }
        fn wrong_middle(address: u64) -> Option<u64> {
            Some(address + if address == 0x2000 { 0x20000 } else { 0x10000 })
        }
        assert!(mapped_region(0x1000, 0x11000, 1024, coherent));
        assert!(!mapped_region(0x1000, 0x21000, 1024, coherent));
        assert!(!mapped_region(0x1000, 0x11000, 8192, missing_end));
        assert!(!mapped_region(0x1000, 0x11000, 8192, wrong_middle));
        assert!(!mapped_region(u64::MAX - 1, 0x11000, 1024, coherent));
        assert!(!mapped_region(0x1000, u64::MAX - 1, 1024, coherent));
    }
}
