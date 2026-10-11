//! Root-retained control + ISO provider; no USB class or PCM interpretation.
#![allow(dead_code)] // Source catalog/product dispatch is integrated independently.
use super::{
    UsbControlCallRefusal, UsbControlSelection, UsbDevice, UsbDma,
    control_installation::{ReadyUsbControlBase, validate_selection},
    device_dma_pointer,
    endpoint_setup::ConfiguredIsochronousOutboundEndpoint,
    endpoint_write_transfer::EndpointTransmitDma,
    native_host::{UsbNativeCallIndex, UsbNativeHost},
    protocol_installation::{UsbNativeProtocolRefusal, mapped_region, native_refusal},
};
use crate::{
    arch::xhci_mechanism::XhciReady,
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::{
        control_contract::ControlContract,
        installation::{UsbInstallationRefusal, UsbNativeIdentity},
        isochronous::{
            contract::*,
            factory::*,
            offer,
            owner::{
                IsochronousAttachment, IsochronousOutCallOwner, IsochronousOwnerRefusal,
                NativeIsochronousSubmission,
            },
            request::DecodedIsochronousRequest,
            result::{IsochronousCompletion, IsochronousOutcome},
        },
        isochronous_schedule::IsochronousOutObservation,
    },
};
use alloc::vec;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[derive(Debug)]
pub enum UsbIsochronousCallRefusal {
    Native(UsbNativeProtocolRefusal),
    Owner(IsochronousOwnerRefusal),
    Binding,
}
impl From<UsbNativeProtocolRefusal> for UsbIsochronousCallRefusal {
    fn from(error: UsbNativeProtocolRefusal) -> Self {
        Self::Native(error)
    }
}
impl From<IsochronousOwnerRefusal> for UsbIsochronousCallRefusal {
    fn from(error: IsochronousOwnerRefusal) -> Self {
        Self::Owner(error)
    }
}
fn binding() -> UsbNativeProtocolRefusal {
    UsbNativeProtocolRefusal::Installation(UsbInstallationRefusal::InvalidIdentity)
}
pub(crate) struct ReadyUsbIsochronousHost {
    control: ReadyUsbControlBase,
    identity: UsbNativeIdentity,
    configured: ConfiguredIsochronousOutboundEndpoint,
    dma: EndpointTransmitDma<'static>,
    attachment: IsochronousAttachment,
}
impl ReadyUsbIsochronousHost {
    /// # Safety
    /// Sole Root owns the original device/controller and coherent disjoint ISO
    /// storage, exact Configure Endpoint receipt and complete preparation envelope.
    /// Every failed admission remains quarantined; no other event consumer exists.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(
        control_identity: UsbNativeIdentity,
        identity: UsbNativeIdentity,
        controller: XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
        configured: ConfiguredIsochronousOutboundEndpoint,
        dma: EndpointTransmitDma<'static>,
        attachment: IsochronousAttachment,
    ) -> Result<Self, UsbNativeProtocolRefusal> {
        control_identity
            .validate()
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        identity
            .validate()
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        let receipt = &configured.configured;
        if identity.host_id != control_identity.host_id
            || identity.boot_id != control_identity.boot_id
            || identity.artifact_id != control_identity.artifact_id
            || identity.provider_generation != u64::from(device.attachment_epoch)
            || identity.base_id == control_identity.base_id
            || identity.provider_instance_id == control_identity.provider_instance_id
            || identity.resource_pool_id == control_identity.resource_pool_id
            || receipt.slot != device.slot
            || receipt.device_epoch != device.attachment_epoch
            || receipt.endpoint_epoch != attachment.endpoint_generation
            || receipt.dci != attachment.endpoint_dci
            || receipt.ring_physical != dma.ring_physical
            || attachment.slot != device.slot
            || attachment.generation != identity.provider_generation
            || attachment.maximum_packet_bytes != configured.geometry.packet_bytes()
            || attachment.resource_bytes < ISOCHRONOUS_OUT_RESOURCE_BYTES
            || dma.ring.len() != 64
            || dma.buffer.len() != 2048
            || !mapped_region(dma.ring.as_ptr() as u64, dma.ring_physical, 1024, mapping)
            || !mapped_region(
                dma.buffer.as_ptr() as u64,
                dma.buffer_physical,
                2048,
                mapping,
            )
        {
            return Err(binding());
        }
        let control_dma = device_dma_pointer(&device).map_err(|error| {
            UsbNativeProtocolRefusal::Control(UsbControlCallRefusal::Native(error))
        })?;
        let physical = mapping(control_dma as u64).ok_or_else(binding)?;
        let regions = [
            (physical, core::mem::size_of::<UsbDma>() as u64),
            (dma.ring_physical, 1024),
            (dma.buffer_physical, 2048),
        ];
        if !regions.iter().enumerate().all(|(index, (start, bytes))| {
            start.checked_add(*bytes).is_some_and(|end| {
                regions[..index].iter().all(|(prior, size)| {
                    prior
                        .checked_add(*size)
                        .is_some_and(|prior_end| end <= *prior || prior_end <= *start)
                })
            })
        }) {
            return Err(binding());
        }
        let control =
            unsafe { ReadyUsbControlBase::new(control_identity, controller, device, mapping) }
                .map_err(UsbNativeProtocolRefusal::Installation)?;
        Ok(Self {
            control,
            identity,
            configured,
            dma,
            attachment,
        })
    }
    pub fn control_identity(&self) -> &UsbNativeIdentity {
        self.control.identity()
    }
    pub fn isochronous_identity(&self) -> &UsbNativeIdentity {
        &self.identity
    }
    pub fn attachment_epoch(&self) -> u32 {
        self.control.attachment_epoch()
    }
    pub fn control_resource_bytes(&self) -> u64 {
        self.control.resource_bytes()
    }
    pub fn isochronous_resource_bytes(&self) -> u64 {
        self.attachment.resource_bytes
    }
    pub fn append_to_advertisement(
        &self,
        host: &mut HostAdvertisement,
        control: &ControlContract,
        iso: &IsochronousOutContract,
    ) -> Result<(), UsbNativeProtocolRefusal> {
        let mut projected = host.clone();
        self.control
            .append_to_advertisement(&mut projected, control)
            .map_err(UsbNativeProtocolRefusal::Installation)?;
        let entry = BaseProviderEntry {
            base_id: self.identity.base_id.clone(),
            provider_instance_id: self.identity.provider_instance_id.clone(),
            provider_generation: self.identity.provider_generation,
            implementation_id: ISOCHRONOUS_OUT_BASE.into(),
            mechanism_family: ISOCHRONOUS_OUT_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer::offer(iso, self.identity.artifact_id.clone())],
            resources: vec![resource_offer(
                self.identity.resource_pool_id.as_str(),
                ISOCHRONOUS_OUT_ATTACHMENT,
                1,
            )],
        };
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(projected.capabilities.len() + 1)
                .map_err(|_| binding())?,
            maximum_advertised_resources: u16::try_from(projected.resources.len() + 1)
                .map_err(|_| binding())?,
        })
        .map_err(|_| binding())?;
        registry
            .register(entry)
            .and_then(|()| registry.project_ready_into(&mut projected))
            .map_err(|_| binding())?;
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
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &IsochronousOutContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<NativeUsbIsochronousHost, UsbIsochronousCallRefusal> {
        validate_selection(self.control.identity(), &control_claim, &control)
            .map_err(|error| UsbNativeProtocolRefusal::Control(error))?;
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement_id)
            .ok_or(UsbIsochronousCallRefusal::Binding)?;
        let base = gear
            .base
            .as_ref()
            .ok_or(UsbIsochronousCallRefusal::Binding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != ISOCHRONOUS_OUT_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || base.base_id != self.identity.base_id
            || base.provider_instance_id != self.identity.provider_instance_id
            || base.provider_generation != self.identity.provider_generation
            || base.implementation_id.as_str() != ISOCHRONOUS_OUT_BASE
            || base.mechanism_family.as_str() != ISOCHRONOUS_OUT_ATTACHMENT
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(UsbIsochronousCallRefusal::Binding);
        }
        let control_node = control
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == control.placement)
            .ok_or(UsbIsochronousCallRefusal::Binding)?
            .node;
        let iso_node = selection
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == selection.placement_id)
            .ok_or(UsbIsochronousCallRefusal::Binding)?
            .node;
        let owner = unsafe {
            IsochronousOutCallOwner::bind_admitted(
                table,
                handle,
                claim,
                self.attachment,
                contract,
                selection,
            )
        }?;
        let (controller, device, mapping) = self.control.into_native_parts();
        let mut host =
            unsafe { UsbNativeHost::new(controller, device, mapping) }.map_err(native_refusal)?;
        let control_index =
            unsafe { host.add_control(control_table, control_handle, control_claim, control) }
                .map_err(native_refusal)?;
        unsafe { host.add_isochronous(self.configured, self.dma) }.map_err(native_refusal)?;
        Ok(NativeUsbIsochronousHost {
            host,
            owner,
            control_index,
            control_node,
            iso_node,
            endpoint_epoch: self.attachment.endpoint_generation,
            pending: None,
        })
    }
}
struct PendingBatch {
    submission: NativeIsochronousSubmission,
    completed: [IsochronousCompletion; 8],
    count: usize,
    outcome: Option<IsochronousOutcome>,
}
pub(crate) struct NativeUsbIsochronousHost {
    host: UsbNativeHost<'static>,
    owner: IsochronousOutCallOwner,
    control_index: UsbNativeCallIndex,
    control_node: NodeId,
    iso_node: NodeId,
    endpoint_epoch: u64,
    pending: Option<PendingBatch>,
}
impl NativeUsbIsochronousHost {
    pub fn control_invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbIsochronousCallRefusal> {
        if node != self.control_node || call != HostCallId(0) || self.pending.is_some() {
            return Err(UsbIsochronousCallRefusal::Binding);
        }
        self.host
            .control_invoke(self.control_index, request, input)
            .map_err(native_refusal)
            .map_err(Into::into)
    }
    pub fn isochronous_begin(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), UsbIsochronousCallRefusal> {
        if node != self.iso_node || call != HostCallId(0) || self.pending.is_some() {
            return Err(UsbIsochronousCallRefusal::Binding);
        }
        let submission = self.owner.begin(node, call, request, input)?;
        // Retain the exact submission before any native publication, even on refusal.
        self.pending = Some(PendingBatch {
            submission,
            completed: [IsochronousCompletion::default(); 8],
            count: 0,
            outcome: None,
        });
        let result = self.publish_pending();
        if result.is_err() {
            // Conservative original all-stop also covers partial batch publication.
            // Failure leaves both pending claim and every DMA lease quarantined.
            self.host.cancel().map_err(native_refusal)?;
            let pending = self
                .pending
                .as_ref()
                .ok_or(UsbIsochronousCallRefusal::Binding)?;
            unsafe { self.owner.discard_quiesced(&pending.submission) }?;
            self.pending = None;
        }
        result
    }
    fn publish_pending(&mut self) -> Result<(), UsbIsochronousCallRefusal> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(UsbIsochronousCallRefusal::Binding)?;
        match pending.submission.request() {
            DecodedIsochronousRequest::Observe => {
                let snapshot = self
                    .host
                    .isochronous_snapshot(self.endpoint_epoch)
                    .map_err(native_refusal)?;
                pending.outcome = Some(IsochronousOutcome::Snapshot(snapshot));
            }
            DecodedIsochronousRequest::Drain => {
                self.host
                    .drain_isochronous(self.endpoint_epoch)
                    .map_err(native_refusal)?;
                pending.outcome = Some(IsochronousOutcome::Drained);
            }
            DecodedIsochronousRequest::Batch(batch) => {
                let snapshot = self
                    .host
                    .isochronous_snapshot(self.endpoint_epoch)
                    .map_err(native_refusal)?;
                if snapshot.pending_packets != 0 {
                    return Err(UsbIsochronousCallRefusal::Binding);
                }
                batch
                    .validate_schedule(snapshot.current_microframe, snapshot.scheduling_threshold)
                    .map_err(|error| IsochronousOwnerRefusal::Decode(error))?;
                for index in 0..batch.count() {
                    let (frame, wire) = batch
                        .packet(index)
                        .ok_or(UsbIsochronousCallRefusal::Binding)?;
                    // Native begin rechecks actual MFINDEX/IST for every published TD.
                    unsafe {
                        self.host
                            .begin_isochronous(self.endpoint_epoch, frame, wire)
                    }
                    .map_err(native_refusal)?;
                }
            }
        }
        Ok(())
    }
    pub fn isochronous_poll(&mut self) -> Result<Option<&[u8]>, UsbIsochronousCallRefusal> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(UsbIsochronousCallRefusal::Binding)?;
        if pending.outcome.is_none() {
            let Some(observation) = self
                .host
                .poll_isochronous(self.endpoint_epoch)
                .map_err(native_refusal)?
            else {
                return Ok(None);
            };
            pending.outcome = match observation {
                IsochronousOutObservation::Completed {
                    frame,
                    requested,
                    actual,
                } => {
                    if pending.count >= 8 {
                        return Err(UsbIsochronousCallRefusal::Binding);
                    }
                    pending.completed[pending.count] = IsochronousCompletion {
                        frame,
                        requested,
                        actual,
                    };
                    pending.count += 1;
                    let DecodedIsochronousRequest::Batch(batch) = pending.submission.request()
                    else {
                        return Err(UsbIsochronousCallRefusal::Binding);
                    };
                    (pending.count == batch.count()).then_some(IsochronousOutcome::Completed)
                }
                IsochronousOutObservation::Short {
                    frame,
                    requested,
                    actual,
                } => Some(IsochronousOutcome::Short(IsochronousCompletion {
                    frame,
                    requested,
                    actual,
                })),
                IsochronousOutObservation::MissedService { frame } => {
                    Some(IsochronousOutcome::MissedService { frame })
                }
                IsochronousOutObservation::Underrun => Some(IsochronousOutcome::Underrun),
                IsochronousOutObservation::ProviderLost => Some(IsochronousOutcome::ProviderLost),
                _ => return Err(UsbIsochronousCallRefusal::Binding),
            };
        }
        let Some(outcome) = pending.outcome else {
            return Ok(None);
        };
        // Native poll exposes terminal faults only after exact original all-stop.
        // A completed outcome consists solely of actual correlated TD completions.
        let bytes = unsafe {
            self.owner.finish_quiesced(
                &pending.submission,
                &pending.completed[..pending.count],
                outcome,
            )
        }?;
        self.pending = None;
        Ok(Some(bytes))
    }
    pub fn cancel(&mut self) -> Result<(), UsbIsochronousCallRefusal> {
        self.owner.revoke(self.iso_node, HostCallId(0))?;
        self.host.cancel().map_err(native_refusal)?;
        if let Some(pending) = self.pending.as_ref() {
            unsafe { self.owner.discard_quiesced(&pending.submission) }?;
        }
        self.pending = None;
        Ok(())
    }
}
