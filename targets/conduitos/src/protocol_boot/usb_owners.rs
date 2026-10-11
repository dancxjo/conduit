//! One exclusively reserved native USB attachment; no class policy.
use super::{
    owners::{CLAIMED, OperationAuthority, authority},
    usb_request::UsbProtocolBootRequest,
};
use crate::{
    arch, boot,
    cryptographic_entropy::CryptographicEntropyBase,
    protocol_source::NativeProtocolIssuer,
    usb_base::{control_contract::*, control_factory::*, installation::UsbNativeIdentity},
};
use alloc::format;
use conduit_core::*;
use core::sync::atomic::Ordering;

pub(super) struct NativeUsbOwners {
    pub ready: arch::ReadyUsbControlBase,
    pub issuer: NativeProtocolIssuer,
}

/// # Safety
/// The sole local Root has independently released firmware and approved the
/// electrical attachment. No other native controller or DMA consumer may run.
/// The reservation remains quarantined on failure and is never reassigned.
pub(super) unsafe fn admit(
    record: &boot::BootRecord,
    request: &UsbProtocolBootRequest,
    host: &HostId,
    boot_id: &BootId,
    artifact: &str,
) -> Result<NativeUsbOwners, &'static str> {
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "protocol-resources-already-reserved")?;
    let mut controller =
        arch::initialize_xhci(record.hhdm_offset, boot::executable_physical_address)
            .map_err(|_| "protocol-usb-controller-unavailable")?;
    let device = arch::enumerate_selected_root_port_at_epoch(
        &mut controller,
        boot::executable_physical_address,
        request.root_port,
        request.attachment_epoch,
    )
    .map_err(|_| "protocol-usb-attachment-unavailable")?;
    if device.root_port != request.root_port {
        return Err("protocol-usb-attachment-mismatch");
    }
    let generation = u64::from(device.attachment_epoch);
    let prefix = format!(
        "conduitos/protocol/{}/usb/{}/{}/{}",
        boot_id.as_str(),
        device.root_port,
        device.slot,
        device.attachment_epoch
    );
    let identity = UsbNativeIdentity {
        host_id: host.clone(),
        boot_id: boot_id.clone(),
        base_id: format!("{prefix}/controller").into(),
        provider_instance_id: format!("{prefix}/provider").into(),
        provider_generation: generation,
        resource_pool_id: format!("{prefix}/control-dma").into(),
        resource_generation_id: ResourceGenerationId(format!("{prefix}/control-dma/{generation}")),
        envelope_id: format!("{prefix}/control-envelope").into(),
        artifact_id: artifact.into(),
    };
    let mut capability = authority(
        host,
        boot_id,
        "usb-control",
        (
            &identity.provider_instance_id,
            &identity.resource_pool_id,
            &identity.resource_generation_id,
            &identity.envelope_id,
        ),
        OperationAuthority {
            contract: CONTROL_AUTHORITY,
            call: CONTROL_CALL,
            kind: CONTROL_KIND,
            capability: CONTROL_IMPLEMENTATION,
            bytes: CONTROL_MAXIMUM_BYTES,
            work: 1,
            operations: request.maximum_control_operations,
        },
    );
    capability.base_provider_generation = generation;
    // SAFETY: the actual controller and attachment were exclusively retained
    // above; constructor verifies their DMA extent against the native mapping.
    let ready = unsafe {
        arch::ReadyUsbControlBase::new(
            identity,
            controller,
            device,
            boot::executable_physical_address,
        )
    }
    .map_err(|_| "protocol-usb-identity-invalid")?;
    let source = arch::RdrandEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
    let mut entropy =
        CryptographicEntropyBase::<_, 1>::admit(source).map_err(|_| "protocol-entropy-refused")?;
    // SAFETY: independently retained native facts bound this finite authority;
    // Source discovery, package metadata and planning created no grant.
    let issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
        .map_err(|_| "protocol-usb-issuance-refused")?;
    Ok(NativeUsbOwners { ready, issuer })
}
