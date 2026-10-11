//! Product control installation retains one exclusive native event consumer.
use super::{UsbControlCallRefusal, UsbControlSelection};
use super::{
    UsbDevice, UsbDma, device_dma_pointer,
    native_host::{UsbNativeCallIndex, UsbNativeHost, UsbNativeHostRefusal},
};
use crate::arch::xhci_mechanism::XhciReady;
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

    pub(super) fn device(&self) -> &UsbDevice {
        &self.device
    }
    pub(super) fn into_native_parts(self) -> (XhciReady, UsbDevice, fn(u64) -> Option<u64>) {
        (self.controller, self.device, self.mapping)
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
        let node = selection
            .lowered
            .nodes
            .iter()
            .find(|node| &node.placement_id == selection.placement)
            .ok_or(UsbControlCallRefusal::Ownership(
                ControlOwnerRefusal::WrongBinding,
            ))?
            .node;
        // SAFETY: construction retained the exact owner; validation preserves its
        // native identity across ordinary selection and separate Root issuance.
        let mut host = unsafe { UsbNativeHost::new(self.controller, self.device, self.mapping) }
            .map_err(control_refusal)?;
        // SAFETY: exact identity above binds this selected claim to the actual
        // original provider, resource generation and envelope retained by Host.
        let index = unsafe { host.add_control(table, handle, claim, selection) }
            .map_err(control_refusal)?;
        Ok(NativeUsbControlCall {
            host,
            index,
            node,
            call: HostCallId(0),
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

pub(super) fn validate_selection(
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

/// Selected operation delegates to the sole native attachment/event owner.
pub struct NativeUsbControlCall {
    host: UsbNativeHost<'static>,
    index: UsbNativeCallIndex,
    node: NodeId,
    call: HostCallId,
}
impl NativeUsbControlCall {
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], UsbControlCallRefusal> {
        self.check_binding(node, call)?;
        self.host
            .control_invoke(self.index, request, input)
            .map_err(control_refusal)
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        self.check_binding(node, call)?;
        self.host.cancel().map_err(control_refusal)
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), UsbControlCallRefusal> {
        if node != self.node || call != self.call {
            return Err(UsbControlCallRefusal::Ownership(
                ControlOwnerRefusal::WrongBinding,
            ));
        }
        Ok(())
    }
}

fn control_refusal(error: UsbNativeHostRefusal) -> UsbControlCallRefusal {
    match error {
        UsbNativeHostRefusal::Control(error) => error,
        UsbNativeHostRefusal::Native(error) => UsbControlCallRefusal::Native(error),
        UsbNativeHostRefusal::Pending | UsbNativeHostRefusal::Capacity => {
            UsbControlCallRefusal::Ownership(ControlOwnerRefusal::Pressure)
        }
        UsbNativeHostRefusal::NotReady => {
            UsbControlCallRefusal::Ownership(ControlOwnerRefusal::Possession)
        }
        UsbNativeHostRefusal::Binding
        | UsbNativeHostRefusal::Endpoint(_)
        | UsbNativeHostRefusal::Write(_) => {
            UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding)
        }
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

#[cfg(test)]
mod outbound_refusal_tests {
    use super::*;
    #[test]
    fn control_only_adapter_rejects_an_unexpected_outbound_owner() {
        let refusal = control_refusal(UsbNativeHostRefusal::Write(
            super::super::endpoint_write_transfer::EndpointWriteNativeRefusal::Pending,
        ));
        assert!(matches!(
            refusal,
            UsbControlCallRefusal::Ownership(ControlOwnerRefusal::WrongBinding)
        ));
    }
}
