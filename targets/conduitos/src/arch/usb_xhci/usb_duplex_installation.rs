//! Three ordinary USB transport Backs retained by one product native Host.
#![allow(dead_code)] // Product Root wiring follows this native installation.
use super::{
    UsbControlCallRefusal, UsbControlSelection, UsbDevice, UsbDma,
    control_installation::{ReadyUsbControlBase, validate_selection},
    device_dma_pointer,
    endpoint_read::{EndpointNativeRefusal, EndpointReceiveDma},
    endpoint_setup::{ConfiguredInboundEndpoint, ConfiguredOutboundEndpoint},
    endpoint_write_transfer::{EndpointTransmitDma, EndpointWriteNativeRefusal},
    native_host::{UsbNativeCallIndex, UsbNativeHost},
    protocol_installation::{
        UsbNativeProtocolRefusal, native_refusal, validate_endpoint_selection,
    },
};
use crate::{
    arch::xhci_mechanism::XhciReady,
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        control_contract::ControlContract,
        control_owner::ControlOwnerRefusal,
        endpoint_read_contract::EndpointReadContract,
        endpoint_read_owner::{EndpointReadAttachment, EndpointReadOwnerRefusal},
        endpoint_write_contract::EndpointWriteContract,
        endpoint_write_factory::*,
        endpoint_write_offer,
        endpoint_write_owner::{EndpointWriteAttachment, EndpointWriteOwnerRefusal},
        installation::{UsbInstallationRefusal, UsbNativeIdentity},
    },
};
use alloc::vec;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[path = "usb_duplex_installation/geometry.rs"]
mod geometry;
use geometry::{separate_regions, validate_dma, validate_identities};

pub(crate) struct ReadyUsbDuplexHost {
    control: ReadyUsbControlBase,
    read_identity: UsbNativeIdentity,
    write_identity: UsbNativeIdentity,
    read_configured: ConfiguredInboundEndpoint,
    write_configured: ConfiguredOutboundEndpoint,
    read_dma: EndpointReceiveDma<'static>,
    write_dma: EndpointTransmitDma<'static>,
    read_attachment: EndpointReadAttachment,
    write_attachment: EndpointWriteAttachment,
}
impl ReadyUsbDuplexHost {
    /// Required independent endpoint capacity after actual preparation accounting.
    /// Root selects both finite capacities before advertisement; this function
    /// never changes either authority or resource admission.
    pub fn retained_endpoint_resource_bytes(newly_retained: usize) -> Option<u64> {
        retention::retained_bytes(newly_retained)?
            .checked_add(1)
            .map(|bytes| (bytes / 2) as u64)
    }

    /// # Safety
    /// Sole Root owns the original controller/device under all three exact
    /// native provider/resource generations and envelopes. Both actual configured
    /// receipts and every coherent DMA region remain rooted until acknowledged
    /// stop, including all refusals. No other native event consumer may run.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(
        control_identity: UsbNativeIdentity,
        read_identity: UsbNativeIdentity,
        write_identity: UsbNativeIdentity,
        controller: XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
        read_configured: ConfiguredInboundEndpoint,
        read_dma: EndpointReceiveDma<'static>,
        read_attachment: EndpointReadAttachment,
        write_configured: ConfiguredOutboundEndpoint,
        write_dma: EndpointTransmitDma<'static>,
        write_attachment: EndpointWriteAttachment,
    ) -> Result<Self, UsbNativeProtocolRefusal> {
        validate_identities(
            [&control_identity, &read_identity, &write_identity],
            device.attachment_epoch,
        )?;
        if read_attachment.slot != device.slot
            || read_attachment.generation != u64::from(device.attachment_epoch)
            || read_attachment.endpoint_generation != read_configured.endpoint_epoch
            || read_attachment.endpoint_dci != read_configured.dci
            || read_attachment.maximum_data_bytes == 0
            || read_attachment.maximum_data_bytes > 2048
            || read_attachment.resource_bytes < minimum_endpoint_resource_bytes()
        {
            return Err(read_binding());
        }
        if write_configured.slot != device.slot
            || write_configured.device_epoch != device.attachment_epoch
            || write_configured.endpoint_epoch == 0
            || write_configured.endpoint_epoch != write_attachment.endpoint_generation
            || write_configured.dci != write_attachment.endpoint_dci
            || !(2..=30).contains(&write_configured.dci)
            || !write_configured.dci.is_multiple_of(2)
            || write_configured.ring_physical != write_dma.ring_physical
            || write_attachment.slot != device.slot
            || write_attachment.generation != u64::from(device.attachment_epoch)
            || write_attachment.maximum_data_bytes == 0
            || write_attachment.maximum_data_bytes > 2048
            || write_attachment.resource_bytes < minimum_endpoint_resource_bytes()
            || write_dma.cursor.ordinary_slots() != 63
        {
            return Err(write_binding());
        }
        write_dma.cursor.ensure_idle().map_err(|error| {
            UsbNativeProtocolRefusal::Write(EndpointWriteNativeRefusal::Ring(error))
        })?;
        validate_dma(
            read_dma.ring.as_ptr() as u64,
            read_dma.buffer.as_ptr() as u64,
            read_dma.ring_physical,
            read_dma.buffer_physical,
            mapping,
        )
        .map_err(|()| UsbNativeProtocolRefusal::Endpoint(EndpointNativeRefusal::Mapping))?;
        validate_dma(
            write_dma.ring.as_ptr() as u64,
            write_dma.buffer.as_ptr() as u64,
            write_dma.ring_physical,
            write_dma.buffer_physical,
            mapping,
        )
        .map_err(|()| UsbNativeProtocolRefusal::Write(EndpointWriteNativeRefusal::Mapping))?;
        let control_dma = device_dma_pointer(&device).map_err(|error| {
            UsbNativeProtocolRefusal::Control(UsbControlCallRefusal::Native(error))
        })?;
        let control_physical = mapping(control_dma as u64).ok_or(
            UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::InvalidIdentity),
        )?;
        if !separate_regions(&[
            (control_physical, core::mem::size_of::<UsbDma>() as u64),
            (read_dma.ring_physical, 1024),
            (read_dma.buffer_physical, 2048),
            (write_dma.ring_physical, 1024),
            (write_dma.buffer_physical, 2048),
        ]) {
            return Err(UsbNativeProtocolRefusal::Installation(
                UsbInstallationRefusal::InvalidIdentity,
            ));
        }
        let contract =
            EndpointReadContract::prepare().map_err(UsbNativeProtocolRefusal::Canonical)?;
        super::endpoint_offer::ready_entry_for_identity(
            &read_identity,
            &device,
            &read_configured,
            &read_dma,
            &contract,
        )
        .map_err(UsbNativeProtocolRefusal::EndpointOffer)?;
        // SAFETY: the actual original ownership is retained under the exact Root identity.
        let control =
            unsafe { ReadyUsbControlBase::new(control_identity, controller, device, mapping) }
                .map_err(UsbNativeProtocolRefusal::Installation)?;
        Ok(Self {
            control,
            read_identity,
            write_identity,
            read_configured,
            write_configured,
            read_dma,
            write_dma,
            read_attachment,
            write_attachment,
        })
    }
    pub fn control_identity(&self) -> &UsbNativeIdentity {
        self.control.identity()
    }
    pub fn read_identity(&self) -> &UsbNativeIdentity {
        &self.read_identity
    }
    pub fn write_identity(&self) -> &UsbNativeIdentity {
        &self.write_identity
    }
    pub fn attachment_epoch(&self) -> u32 {
        self.control.attachment_epoch()
    }
    pub fn control_resource_bytes(&self) -> u64 {
        self.control.resource_bytes()
    }
    pub fn read_resource_bytes(&self) -> u64 {
        self.read_attachment.resource_bytes
    }
    pub fn write_resource_bytes(&self) -> u64 {
        self.write_attachment.resource_bytes
    }

    /// Atomically describe three actual ready transports without granting possession.
    pub fn append_to_advertisement(
        &self,
        host: &mut HostAdvertisement,
        control_contract: &ControlContract,
        read_contract: &EndpointReadContract,
        write_contract: &EndpointWriteContract,
    ) -> Result<(), UsbNativeProtocolRefusal> {
        let read = super::endpoint_offer::ready_entry_for_identity(
            &self.read_identity,
            self.control.device(),
            &self.read_configured,
            &self.read_dma,
            read_contract,
        )
        .map_err(UsbNativeProtocolRefusal::EndpointOffer)?;
        for identity in [&self.read_identity, &self.write_identity] {
            if host.bases.iter().any(|base| {
                base.base_id == identity.base_id
                    || base.provider_instance_id == identity.provider_instance_id
            }) || host
                .resources
                .iter()
                .any(|resource| resource.pool_id == identity.resource_pool_id)
            {
                return Err(advertisement_refusal());
            }
        }
        if host.capabilities.iter().any(|offer| {
            offer.capability_id.as_str() == ENDPOINT_WRITE_IMPLEMENTATION
                || offer.capability_id == read.capabilities[0].capability_id
        }) {
            return Err(advertisement_refusal());
        }
        let write = BaseProviderEntry {
            base_id: self.write_identity.base_id.clone(),
            provider_instance_id: self.write_identity.provider_instance_id.clone(),
            provider_generation: self.write_identity.provider_generation,
            implementation_id: ENDPOINT_WRITE_BASE.into(),
            mechanism_family: ENDPOINT_WRITE_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![endpoint_write_offer::offer(
                write_contract,
                self.write_identity.artifact_id.clone(),
            )],
            resources: vec![resource_offer(
                self.write_identity.resource_pool_id.as_str(),
                ENDPOINT_WRITE_ATTACHMENT,
                1,
            )],
        };
        let mut projected = host.clone();
        self.control
            .append_to_advertisement(&mut projected, control_contract)
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 2,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                projected.capabilities.len().saturating_add(2),
            )
            .map_err(|_| advertisement_refusal())?,
            maximum_advertised_resources: u16::try_from(
                projected.resources.len().saturating_add(2),
            )
            .map_err(|_| advertisement_refusal())?,
        })
        .map_err(|_| advertisement_refusal())?;
        registry
            .register(read)
            .and_then(|()| registry.register(write))
            .and_then(|()| registry.project_ready_into(&mut projected))
            .map_err(|_| advertisement_refusal())?;
        *host = projected;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bind_selected(
        self,
        control_table: BaseCapabilityTable,
        control_handle: BaseCapabilityHandle,
        control_claim: BaseOperationClaim,
        control: UsbControlSelection<'_>,
        read_table: BaseCapabilityTable,
        read_handle: BaseCapabilityHandle,
        read_claim: BaseOperationClaim,
        read_contract: &EndpointReadContract,
        read: SelectedOperationPlan<'_>,
        write_table: BaseCapabilityTable,
        write_handle: BaseCapabilityHandle,
        write_claim: BaseOperationClaim,
        write_contract: &EndpointWriteContract,
        write: SelectedOperationPlan<'_>,
    ) -> Result<NativeUsbDuplexHost, UsbNativeProtocolRefusal> {
        validate_selection(self.control.identity(), &control_claim, &control)
            .map_err(UsbNativeProtocolRefusal::Control)?;
        validate_endpoint_selection(&self.read_identity, &read_claim, &read)?;
        validate_write_selection(&self.write_identity, &write_claim, &write)?;
        let control_node = control
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == control.placement)
            .ok_or(UsbNativeProtocolRefusal::Control(
                UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding),
            ))?
            .node;
        let read_node = read
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == read.placement_id)
            .ok_or_else(read_binding)?
            .node;
        let write_node = write
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == write.placement_id)
            .ok_or_else(write_binding)?
            .node;
        for (table, claim) in [
            (&control_table, &control_claim),
            (&read_table, &read_claim),
            (&write_table, &write_claim),
        ] {
            retention::validate_metadata(table, claim)?;
        }
        let arena = &crate::allocation::BOOT_ARENA;
        let before = arena.live_bytes();
        let (controller, device, mapping) = self.control.into_native_parts();
        // SAFETY: every selected exact identity preserves independently retained
        // Root ownership; bind functions do not discover or manufacture authority.
        let mut host =
            unsafe { UsbNativeHost::new(controller, device, mapping) }.map_err(native_refusal)?;
        let control_index =
            unsafe { host.add_control(control_table, control_handle, control_claim, control) }
                .map_err(native_refusal)?;
        let read_index = unsafe {
            host.add_endpoint(
                self.read_configured,
                self.read_dma,
                self.read_attachment,
                read_table,
                read_handle,
                read_claim,
                read_contract,
                read,
            )
        }
        .map_err(native_refusal)?;
        let write_index = unsafe {
            host.add_write(
                self.write_configured,
                self.write_dma,
                self.write_attachment,
                write_table,
                write_handle,
                write_claim,
                write_contract,
                write,
            )
        }
        .map_err(native_refusal)?;
        host.prepare_independent().map_err(native_refusal)?;
        let newly_retained = arena
            .live_bytes()
            .checked_sub(before)
            .ok_or_else(read_binding)?;
        if arena.capacity() != 0 {
            retention::admit(
                newly_retained,
                [
                    self.read_attachment.resource_bytes,
                    self.write_attachment.resource_bytes,
                ],
            )?;
        }
        Ok(NativeUsbDuplexHost {
            host,
            control_index,
            read_index,
            write_index,
            control_node,
            read_node,
            write_node,
        })
    }
}

pub(crate) struct NativeUsbDuplexHost {
    host: UsbNativeHost<'static>,
    control_index: UsbNativeCallIndex,
    read_index: UsbNativeCallIndex,
    write_index: UsbNativeCallIndex,
    control_node: NodeId,
    read_node: NodeId,
    write_node: NodeId,
}
#[path = "usb_duplex_installation/runtime.rs"]
mod runtime;

fn validate_write_selection(
    identity: &UsbNativeIdentity,
    claim: &BaseOperationClaim,
    selection: &SelectedOperationPlan<'_>,
) -> Result<(), UsbNativeProtocolRefusal> {
    let gear = selection
        .fragment
        .placements
        .iter()
        .find(|gear| &gear.placement_id == selection.placement_id)
        .ok_or_else(write_binding)?;
    let base = gear.base.as_ref().ok_or_else(write_binding)?;
    if claim.host_id != identity.host_id
        || claim.boot_id != identity.boot_id
        || claim.base_instance_id != identity.provider_instance_id
        || claim.base_provider_generation != identity.provider_generation
        || claim.resource_pool_id != identity.resource_pool_id
        || claim.resource_generation_id != identity.resource_generation_id
        || claim.envelope_id != identity.envelope_id
        || claim.implementation_id.as_str() != ENDPOINT_WRITE_IMPLEMENTATION
        || gear.artifact_id != identity.artifact_id
        || base.base_id != identity.base_id
        || base.provider_instance_id != identity.provider_instance_id
        || base.provider_generation != identity.provider_generation
        || base.implementation_id.as_str() != ENDPOINT_WRITE_BASE
        || base.mechanism_family.as_str() != ENDPOINT_WRITE_ATTACHMENT
        || base.enforcement_class != BaseEnforcementClass::Cooperative
    {
        return Err(write_binding());
    }
    Ok(())
}
fn read_binding() -> UsbNativeProtocolRefusal {
    UsbNativeProtocolRefusal::Endpoint(EndpointNativeRefusal::Ownership(
        EndpointReadOwnerRefusal::WrongBinding,
    ))
}
fn write_binding() -> UsbNativeProtocolRefusal {
    UsbNativeProtocolRefusal::Write(EndpointWriteNativeRefusal::Ownership(
        EndpointWriteOwnerRefusal::WrongBinding,
    ))
}
fn advertisement_refusal() -> UsbNativeProtocolRefusal {
    UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::Advertisement)
}

/// Each independently admitted endpoint covers its DMA and fixed terminal output.
pub(crate) const fn minimum_endpoint_resource_bytes() -> u64 {
    super::native_host::independent_endpoint_resource_bytes()
}

#[path = "usb_duplex_installation/retention.rs"]
mod retention;
