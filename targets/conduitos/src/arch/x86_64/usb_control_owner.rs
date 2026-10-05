//! Selected control possession joined to exclusively retained native machinery.
use conduit_composite::KernelOperationFactory;
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable,
    BaseOperationClaim, PlacementId, PlanFragment,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

use super::{
    UsbDevice, UsbDma, UsbError,
    control::{ControlRing, control_raw},
    device_dma_pointer,
};
use crate::arch::x86_64::xhci::XhciReady;
use crate::machine_membrane::selected_operation::SelectedOperationPlan;
use crate::usb_base::{
    control_contract::ControlContract,
    control_factory::ControlOperationFactory,
    control_owner::{
        ControlAttachment, ControlCallOwner, ControlOwnerRefusal, NativeControlObservation,
        NativeControlSubmission,
    },
};

pub struct UsbControlSelection<'a> {
    pub contract: &'a ControlContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UsbControlCallRefusal {
    Ownership(ControlOwnerRefusal),
    Native(UsbError),
}

/// One selected endpoint; requests cannot substitute its slot, port or DMA.
/// Dropping this value does not acknowledge hardware stop or free static DMA.
pub struct UsbControlHostCall<'a> {
    controller: &'a mut XhciReady,
    device: UsbDevice,
    ring: ControlRing,
    owner: ControlCallOwner,
    pending: Option<NativeControlSubmission>,
}

impl<'a> UsbControlHostCall<'a> {
    /// Join actual native ownership to already admitted Plan/Play possession.
    ///
    /// # Safety
    /// The trusted composition root must own this exact controller, device and
    /// coherent DMA mapping under the claim's provider and resource generation.
    /// The image mapping must remain valid until acknowledged endpoint stop.
    /// Discovery and descriptive IDs do not establish these facts. Native code
    /// sharing this address space is not confined by this cooperative membrane.
    #[allow(clippy::too_many_arguments)] // Keep native ownership and selected possession explicit at one safety boundary.
    pub unsafe fn bind_selected(
        controller: &'a mut XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: UsbControlSelection<'_>,
    ) -> Result<Self, UsbControlCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(UsbControlCallRefusal::Ownership(
                ControlOwnerRefusal::WrongBinding,
            ))?;
        ControlOperationFactory::prepare_contract()
            .map_err(|error| {
                UsbControlCallRefusal::Ownership(ControlOwnerRefusal::Canonical(error))
            })?
            .budget(gear)
            .map_err(|_| UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding))?;
        let native = |error| UsbControlCallRefusal::Native(error);
        let dma = device_dma_pointer(&device).map_err(native)?;
        unsafe { (*dma).control_cursor.ensure_idle() }
            .map_err(|error| native(super::control::ring_refusal(error)))?;
        if device.root_port == 0 || device.root_port > controller.maximum_ports() {
            return Err(native(UsbError::RootPortInvalid));
        }
        let start = dma as u64;
        let bytes = core::mem::size_of::<UsbDma>() as u64;
        let physical = mapped_dma_physical(start, mapping).map_err(native)?;
        let owner = unsafe {
            ControlCallOwner::bind_admitted(
                table,
                handle,
                claim,
                ControlAttachment {
                    slot: device.slot,
                    generation: u64::from(device.attachment_epoch),
                    maximum_data_bytes: super::MAX_CONFIGURATION_BYTES as u16,
                    resource_bytes: bytes,
                },
                selection.contract,
                SelectedOperationPlan {
                    fragment: selection.fragment,
                    lowered: selection.lowered,
                    active: selection.active,
                    placement_id: selection.placement,
                },
            )
        }
        .map_err(UsbControlCallRefusal::Ownership)?;
        let ring = ControlRing {
            physical: physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64,
            buffer_physical: physical + core::mem::offset_of!(UsbDma, descriptor) as u64,
            root_port: device.root_port,
            slot: device.slot,
            short_packets: 0,
            dma,
        };
        Ok(Self {
            controller,
            device,
            ring,
            owner,
            pending: None,
        })
    }

    /// Dispatch one kernel request with finite native polling and no retries.
    /// On unacknowledged failure the submission and DMA remain retained.
    pub fn execute(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbControlCallRefusal> {
        let dma = device_dma_pointer(&self.device).map_err(UsbControlCallRefusal::Native)?;
        let submission = self
            .owner
            .begin(node, call, request_id, input)
            .map_err(UsbControlCallRefusal::Ownership)?;
        self.pending = Some(submission);
        let submission = self.pending.as_ref().expect("accepted submission retained");
        let request = submission.request().map_err(|reason| {
            UsbControlCallRefusal::Ownership(ControlOwnerRefusal::Decode(
                crate::usb_base::control_decode::ControlDecodeRefusal::Transfer(reason),
            ))
        })?;
        let is_input = request.input();
        let actual = control_raw(self.controller, &mut self.ring, self.device.slot, request)
            .map_err(UsbControlCallRefusal::Native)?;
        let submission = self.pending.take().expect("completed submission retained");
        let input = if is_input {
            // The exact final status completed; fixed DMA remains exclusively owned.
            unsafe { &(&(*dma).descriptor)[..actual] }
        } else {
            &[]
        };
        unsafe {
            self.owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: actual as u16,
                    input,
                },
            )
        }
        .map_err(UsbControlCallRefusal::Ownership)
    }

    /// Revoke software authority, then acknowledge native slot stop before release.
    /// A failed stop retains both the pending operation and static DMA ownership.
    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(UsbControlCallRefusal::Ownership)?;
        let dma = device_dma_pointer(&self.device).map_err(UsbControlCallRefusal::Native)?;
        self.controller
            .disable_removed_slot(self.device.slot)
            .map_err(|error| UsbControlCallRefusal::Native(error.into()))?;
        // Disable Slot acknowledges all endpoint DMA stop, even after timeout.
        unsafe {
            (*dma).owner_slot = 0;
        }
        if let Some(submission) = self.pending.take() {
            match unsafe { self.owner.discard_quiesced(&submission) } {
                Ok(())
                | Err(ControlOwnerRefusal::Capability(BaseCapabilityRefusal::StaleCompletion)) => {}
                Err(error) => return Err(UsbControlCallRefusal::Ownership(error)),
            }
        }
        Ok(())
    }

    /// End software possession only when every submitted transfer is quiescent.
    /// The native composition root receives the existing attachment for its next
    /// separately admitted use; this does not issue a new capability or reset DMA.
    pub fn release_completed(
        mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<UsbDevice, UsbControlCallRefusal> {
        if self.pending.is_some() {
            return Err(UsbControlCallRefusal::Native(
                UsbError::TransferRingUncertain,
            ));
        }
        device_dma_pointer(&self.device).map_err(UsbControlCallRefusal::Native)?;
        self.owner
            .revoke(node, call)
            .map_err(UsbControlCallRefusal::Ownership)?;
        Ok(self.device)
    }
}

fn mapped_dma_physical(start: u64, mapping: fn(u64) -> Option<u64>) -> Result<u64, UsbError> {
    let bytes = core::mem::size_of::<UsbDma>() as u64;
    let physical = mapping(start).ok_or(UsbError::DmaAddressInvalid)?;
    let last = start
        .checked_add(bytes - 1)
        .ok_or(UsbError::DmaAddressInvalid)?;
    let physical_last = physical
        .checked_add(bytes - 1)
        .ok_or(UsbError::DmaAddressInvalid)?;
    if start & 0xfff != 0 || physical & 0xfff != 0 || mapping(last) != Some(physical_last) {
        return Err(UsbError::DmaAddressInvalid);
    }
    for offset in (0..bytes).step_by(4096) {
        if mapping(start + offset) != Some(physical + offset) {
            return Err(UsbError::DmaAddressInvalid);
        }
    }
    Ok(physical)
}

#[cfg(test)]
#[path = "usb_control_owner_tests.rs"]
mod tests;
