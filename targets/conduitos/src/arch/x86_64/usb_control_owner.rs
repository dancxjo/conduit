//! Selected control possession joined to exclusively retained native machinery.
use super::UsbDevice;
#[cfg(test)]
use super::UsbError;
use crate::arch::x86_64::xhci::XhciReady;
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};

#[path = "usb_control_transfer.rs"]
mod transfer;
pub(super) use transfer::UsbControlTransfer;
#[cfg(test)]
use transfer::mapped_dma_physical;
pub use transfer::{UsbControlCallRefusal, UsbControlSelection};

/// Exclusive controller adapter for a single selected control operation.
/// The transfer state can also be retained by a product controller dispatcher;
/// it does not acquire a second controller reference or event consumer.
pub struct UsbControlHostCall<'a> {
    controller: &'a mut XhciReady,
    transfer: UsbControlTransfer,
}

impl<'a> UsbControlHostCall<'a> {
    /// # Safety
    /// Root exclusively owns this controller, attachment and coherent DMA under
    /// the selected claim. Mapping/storage remain valid until acknowledged stop.
    /// The controller must have no other event consumer while this adapter lives.
    /// Discovery and descriptive identities do not establish possession; this
    /// cooperative membrane does not confine native code in the same address space.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn bind_selected(
        controller: &'a mut XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: UsbControlSelection<'_>,
    ) -> Result<Self, UsbControlCallRefusal> {
        let transfer = unsafe {
            UsbControlTransfer::bind_selected(
                controller, device, mapping, table, handle, claim, selection,
            )
        }?;
        Ok(Self {
            controller,
            transfer,
        })
    }

    pub fn execute(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbControlCallRefusal> {
        // SAFETY: this adapter retains the original exclusive controller borrow.
        unsafe {
            self.transfer
                .execute(self.controller, node, call, request, input)
        }
    }

    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        // SAFETY: this adapter retains the original exclusive controller borrow.
        unsafe { self.transfer.cancel(self.controller, node, call) }
    }

    pub fn release_completed(
        self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<UsbDevice, UsbControlCallRefusal> {
        self.transfer.release_completed(node, call)
    }
}

#[cfg(test)]
#[path = "usb_control_owner_tests.rs"]
mod tests;
