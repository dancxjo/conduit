//! Product control installation retains one exclusive native event consumer.
use super::{UsbControlCallRefusal, UsbControlSelection};
use super::{UsbDevice, UsbDma, control_owner::UsbControlTransfer, device_dma_pointer};
use crate::arch::x86_64::xhci::XhciReady;
use crate::usb_base::{
    control_contract::ControlContract,
    control_factory::*,
    control_owner::ControlOwnerRefusal,
    installation::{UsbInstallationRefusal, UsbNativeIdentity},
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};

pub struct ReadyUsbControlBase {
    identity: UsbNativeIdentity,
    controller: XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
}

impl ReadyUsbControlBase {
    /// Retain the actual native controller and attachment before ordinary planning.
    ///
    /// # Safety
    /// Root exclusively owns this controller, attachment and coherent DMA mapping
    /// under exactly this provider/resource generation and envelope. Mapping and
    /// storage remain valid until acknowledged physical stop, including failures.
    /// No other consumer may drain controller events. Descriptive discovery does
    /// not grant ownership or contain native code sharing this address space.
    pub unsafe fn new(
        identity: UsbNativeIdentity,
        controller: XhciReady,
        device: UsbDevice,
        mapping: fn(u64) -> Option<u64>,
    ) -> Result<Self, UsbInstallationRefusal> {
        identity.validate()?;
        validate_attachment_identity(
            identity.provider_generation,
            device.attachment_epoch,
            device.slot,
            device.root_port,
            controller.maximum_ports(),
        )?;
        let dma =
            device_dma_pointer(&device).map_err(|_| UsbInstallationRefusal::InvalidIdentity)?;
        // Validate the actual retained attachment's full contiguous DMA mapping.
        let start = dma as u64;
        let bytes = core::mem::size_of::<UsbDma>() as u64;
        let physical = mapping(start).ok_or(UsbInstallationRefusal::InvalidIdentity)?;
        let last = start
            .checked_add(bytes - 1)
            .ok_or(UsbInstallationRefusal::InvalidIdentity)?;
        let physical_last = physical
            .checked_add(bytes - 1)
            .ok_or(UsbInstallationRefusal::InvalidIdentity)?;
        if start & 0xfff != 0
            || physical & 0xfff != 0
            || mapping(last) != Some(physical_last)
            || (0..bytes)
                .step_by(4096)
                .any(|offset| mapping(start + offset) != Some(physical + offset))
        {
            return Err(UsbInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            controller,
            device,
            mapping,
        })
    }

    pub fn identity(&self) -> &UsbNativeIdentity {
        &self.identity
    }
    pub fn attachment_epoch(&self) -> u32 {
        self.device.attachment_epoch
    }
    pub fn resource_bytes(&self) -> u64 {
        core::mem::size_of::<UsbDma>() as u64
    }
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &ControlContract,
    ) -> Result<(), UsbInstallationRefusal> {
        self.identity
            .append_to_advertisement(advertisement, contract)
    }

    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: UsbControlSelection<'_>,
    ) -> Result<NativeUsbControlCall, UsbControlCallRefusal> {
        validate_selection(&self.identity, &claim, &selection)?;
        // SAFETY: construction retained the exact owner; validation preserves its
        // native identity across ordinary selection and separate Root issuance.
        let transfer = unsafe {
            UsbControlTransfer::bind_selected(
                &self.controller,
                self.device,
                self.mapping,
                table,
                handle,
                claim,
                selection,
            )
        }?;
        Ok(NativeUsbControlCall {
            controller: self.controller,
            transfer,
        })
    }
}

/// Provider generations represent the exact retained native attachment epoch.
fn validate_attachment_identity(
    provider_generation: u64,
    attachment_epoch: u32,
    slot: u8,
    root_port: u8,
    maximum_ports: u8,
) -> Result<(), UsbInstallationRefusal> {
    if attachment_epoch == 0
        || provider_generation != u64::from(attachment_epoch)
        || slot == 0
        || root_port == 0
        || root_port > maximum_ports
    {
        return Err(UsbInstallationRefusal::InvalidIdentity);
    }
    Ok(())
}

fn validate_selection(
    identity: &UsbNativeIdentity,
    claim: &BaseOperationClaim,
    selection: &UsbControlSelection<'_>,
) -> Result<(), UsbControlCallRefusal> {
    let refuse = || UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding);
    let gear = selection
        .fragment
        .placements
        .iter()
        .find(|gear| &gear.placement_id == selection.placement)
        .ok_or_else(refuse)?;
    let base = gear.base.as_ref().ok_or_else(refuse)?;
    validate_claim(identity, claim)?;
    if gear.artifact_id != identity.artifact_id
        || base.base_id != identity.base_id
        || base.provider_instance_id != identity.provider_instance_id
        || base.provider_generation != identity.provider_generation
        || base.implementation_id.as_str() != CONTROL_BASE
        || base.mechanism_family.as_str() != CONTROL_ATTACHMENT
        || base.enforcement_class != BaseEnforcementClass::Cooperative
    {
        return Err(refuse());
    }
    Ok(())
}

/// Selected operation owns the original exclusive controller, without a borrow.
pub struct NativeUsbControlCall {
    controller: XhciReady,
    transfer: UsbControlTransfer,
}
impl NativeUsbControlCall {
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbControlCallRefusal> {
        // SAFETY: original exclusive controller and mapping remain retained.
        unsafe {
            self.transfer
                .execute(&mut self.controller, node, call, request, input)
        }
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        // SAFETY: cancellation acknowledges physical stop on the original owner.
        unsafe { self.transfer.cancel(&mut self.controller, node, call) }
    }
}

fn validate_claim(
    identity: &UsbNativeIdentity,
    claim: &BaseOperationClaim,
) -> Result<(), UsbControlCallRefusal> {
    if claim.host_id != identity.host_id
        || claim.boot_id != identity.boot_id
        || claim.base_instance_id != identity.provider_instance_id
        || claim.base_provider_generation != identity.provider_generation
        || claim.resource_pool_id != identity.resource_pool_id
        || claim.resource_generation_id != identity.resource_generation_id
        || claim.envelope_id != identity.envelope_id
        || claim.implementation_id.as_str() != CONTROL_IMPLEMENTATION
    {
        return Err(UsbControlCallRefusal::Ownership(
            ControlOwnerRefusal::WrongBinding,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "usb_control_installation_tests.rs"]
mod tests;
