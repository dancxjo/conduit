//! Selected control possession joined to exclusively retained native machinery.
use conduit_composite::KernelOperationFactory;
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable,
    BaseOperationClaim, PlacementId, PlanFragment,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

use super::super::{
    UsbDevice, UsbDma, UsbError,
    control::{ControlRing, control_raw_continuous},
    device_dma_pointer,
};
use crate::arch::xhci_mechanism::XhciReady;
use crate::machine_membrane::selected_operation::SelectedOperationPlan;
use crate::usb_base::{
    control_contract::ControlContract,
    control_factory::ControlOperationFactory,
    control_owner::{
        ControlAttachment, ControlCallOwner, ControlOwnerRefusal, NativeControlObservation,
        NativeControlSubmission,
    },
    control_result::ControlTransferDisposition,
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
pub(in crate::arch::usb_xhci) struct UsbControlTransfer {
    attachment: NativeAttachment,
    ring: ControlRing,
    owner: ControlCallOwner,
    pending: Option<NativeControlSubmission>,
    stalled: bool,
}

/// Private structural snapshot checked against the Root-retained move-only device
/// and its live DMA owner. These facts alone never establish native ownership.
struct NativeAttachment {
    slot: u8,
    root_port: u8,
    dma_slot: u8,
    epoch: u32,
}

impl UsbControlTransfer {
    #[cfg(test)]
    pub(in crate::arch::usb_xhci) fn retains_native_submission(&self) -> bool {
        self.pending.is_some()
    }

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
        controller: &XhciReady,
        device: &UsbDevice,
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
        let dma = device_dma_pointer(device).map_err(native)?;
        unsafe { (*dma).control_cursor.ensure_idle() }
            .map_err(|error| native(super::super::control::ring_refusal(error)))?;
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
                    maximum_data_bytes: super::super::MAX_CONFIGURATION_BYTES as u16,
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
            attachment: NativeAttachment {
                slot: device.slot,
                root_port: device.root_port,
                dma_slot: device.dma_slot,
                epoch: device.attachment_epoch,
            },
            ring,
            owner,
            pending: None,
            stalled: false,
        })
    }

    /// Dispatch one kernel request with finite native polling and no retries.
    /// On unacknowledged failure the submission and DMA remain retained.
    /// # Safety
    /// Caller supplies the same exclusively owned controller and the original
    /// Root-retained move-only device used at binding, with no concurrent event
    /// consumer. Copied structural facts cannot establish this ownership. The
    /// original mapping and DMA remain retained through acknowledged stop.
    pub unsafe fn execute(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbControlCallRefusal> {
        let dma = self.check_attachment(device)?;
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
        let actual =
            match control_raw_continuous(controller, &mut self.ring, self.attachment.slot, request)
            {
                Ok(actual) => actual,
                Err(error) => {
                    // Only the exact correlated native STALL can become a typed
                    // stalled observation. Every refusal retains the submission.
                    self.stalled = is_terminal_stall(error);
                    return Err(UsbControlCallRefusal::Native(error));
                }
            };
        super::attachment::validate_attachment_continuity(controller.port_status(device.root_port))
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

    /// Complete an exact retained STALL only after original native stop.
    /// # Safety
    /// The sole Host acknowledged Disable Slot for this original attachment;
    /// all DMA is quiescent, its ownership marker is still live, and this
    /// submission's software lease has not been revoked.
    pub unsafe fn finish_stalled_quiesced(
        &mut self,
        device: &UsbDevice,
    ) -> Result<&[u8], UsbControlCallRefusal> {
        self.check_attachment(device)?;
        if !self.stalled {
            return Err(UsbControlCallRefusal::Ownership(
                ControlOwnerRefusal::WrongBinding,
            ));
        }
        let submission = self.pending.take().ok_or(UsbControlCallRefusal::Ownership(
            ControlOwnerRefusal::WrongBinding,
        ))?;
        self.stalled = false;
        unsafe {
            self.owner.finish_quiesced(
                &submission,
                NativeControlObservation::Disposition(ControlTransferDisposition::Stalled),
            )
        }
        .map_err(UsbControlCallRefusal::Ownership)
    }

    /// Revoke software possession without claiming native stop or releasing DMA.
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(UsbControlCallRefusal::Ownership)
    }

    /// Release retained submissions only after the shared Root acknowledged stop.
    /// This does not disable the slot or clear the shared native ownership marker.
    ///
    /// # Safety
    /// Root must have acknowledged physical stop of this exact original device,
    /// including all endpoint DMA, through the original exclusive controller.
    /// It must retain the move-only attachment and mapping, with the live DMA
    /// ownership marker intact until every shared transfer has released its lease.
    /// A copied descriptor snapshot, timeout or software revoke is insufficient.
    pub unsafe fn release_quiesced(
        &mut self,
        device: &UsbDevice,
    ) -> Result<(), UsbControlCallRefusal> {
        self.check_attachment(device)?;
        if let Some(submission) = self.pending.as_ref() {
            match unsafe { self.owner.discard_quiesced(submission) } {
                Ok(())
                | Err(ControlOwnerRefusal::Capability(BaseCapabilityRefusal::StaleCompletion)) => {}
                Err(error) => return Err(UsbControlCallRefusal::Ownership(error)),
            }
            self.pending = None;
        }
        Ok(())
    }

    fn check_attachment(&self, device: &UsbDevice) -> Result<*mut UsbDma, UsbControlCallRefusal> {
        if device.slot != self.attachment.slot
            || device.root_port != self.attachment.root_port
            || device.dma_slot != self.attachment.dma_slot
            || device.attachment_epoch != self.attachment.epoch
        {
            return Err(UsbControlCallRefusal::Native(UsbError::StaleDeviceInstance));
        }
        let dma = device_dma_pointer(device).map_err(UsbControlCallRefusal::Native)?;
        if dma != self.ring.dma {
            return Err(UsbControlCallRefusal::Native(UsbError::StaleDeviceInstance));
        }
        Ok(dma)
    }

    /// End software possession only when every submitted transfer is quiescent.
    /// The native composition root receives the existing attachment for its next
    /// separately admitted use; this does not issue a new capability or reset DMA.
    pub fn release_completed(
        mut self,
        device: &UsbDevice,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), UsbControlCallRefusal> {
        if self.pending.is_some() {
            return Err(UsbControlCallRefusal::Native(
                UsbError::TransferRingUncertain,
            ));
        }
        self.check_attachment(device)?;
        self.owner
            .revoke(node, call)
            .map_err(UsbControlCallRefusal::Ownership)?;
        Ok(())
    }
}

pub(in crate::arch::usb_xhci) fn mapped_dma_physical(
    start: u64,
    mapping: fn(u64) -> Option<u64>,
) -> Result<u64, UsbError> {
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

fn is_terminal_stall(error: UsbError) -> bool {
    error == UsbError::ControlStall
}

#[cfg(test)]
mod terminal_tests {
    use super::*;
    #[test]
    fn only_exact_native_stall_can_become_a_typed_stalled_observation() {
        assert!(is_terminal_stall(UsbError::ControlStall));
        for error in [
            UsbError::ControlTimeout,
            UsbError::DeviceVanished,
            UsbError::WrongController,
            UsbError::WrongSlot,
            UsbError::WrongEndpoint,
            UsbError::MalformedCompletion,
            UsbError::ControlError,
            UsbError::TransferRingUncertain,
            UsbError::StaleDeviceInstance,
        ] {
            assert!(!is_terminal_stall(error));
        }
    }
}
