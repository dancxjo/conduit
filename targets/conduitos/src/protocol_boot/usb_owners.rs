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
use alloc::{boxed::Box, format};
use conduit_core::*;
use core::sync::atomic::Ordering;

#[path = "usb_endpoint_owners.rs"]
mod endpoint;

pub(super) enum NativeUsbOwners {
    Control {
        ready: Box<arch::ReadyUsbControlBase>,
        issuer: Box<NativeProtocolIssuer>,
    },
    Protocol {
        ready: Box<arch::ReadyUsbProtocolHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        endpoint_issuer: Box<NativeProtocolIssuer>,
    },
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
        arch::initialize_xhci(record.hhdm_offset, boot::executable_physical_address).map_err(
            |error| {
                native_refusal(
                    "controller",
                    error.as_str(),
                    request.root_port,
                    request.attachment_epoch,
                )
            },
        )?;
    let device = arch::address_selected_root_port_at_epoch(
        &mut controller,
        boot::executable_physical_address,
        request.root_port,
        request.attachment_epoch,
    )
    .map_err(|error| {
        native_refusal(
            "attachment",
            error.as_str(),
            request.root_port,
            request.attachment_epoch,
        )
    })?;
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
    if let Some(endpoint_request) = request.endpoint_read.as_ref() {
        // SAFETY: the sole Root reservation and administrator parameters bind
        // this exact controller/attachment; static DMA remains quarantined.
        let endpoint = unsafe { endpoint::configure(&mut controller, &device, endpoint_request) }?;
        let endpoint_identity = UsbNativeIdentity {
            host_id: host.clone(),
            boot_id: boot_id.clone(),
            base_id: format!("{prefix}/endpoint-controller").into(),
            provider_instance_id: format!("{prefix}/endpoint-provider").into(),
            provider_generation: generation,
            resource_pool_id: format!("{prefix}/endpoint-dma/{}", endpoint_request.endpoint_epoch)
                .into(),
            resource_generation_id: ResourceGenerationId(format!(
                "{prefix}/endpoint-dma/{}/{}",
                generation, endpoint_request.endpoint_epoch
            )),
            envelope_id: format!(
                "{prefix}/endpoint-envelope/{}",
                endpoint_request.endpoint_epoch
            )
            .into(),
            artifact_id: artifact.into(),
        };
        use crate::usb_base::{endpoint_read_contract::*, endpoint_read_factory::*};
        let mut endpoint_capability = authority(
            host,
            boot_id,
            "usb-endpoint-read",
            (
                &endpoint_identity.provider_instance_id,
                &endpoint_identity.resource_pool_id,
                &endpoint_identity.resource_generation_id,
                &endpoint_identity.envelope_id,
            ),
            OperationAuthority {
                contract: ENDPOINT_READ_AUTHORITY,
                call: ENDPOINT_READ_CALL,
                kind: ENDPOINT_READ_KIND,
                capability: ENDPOINT_READ_IMPLEMENTATION,
                bytes: ENDPOINT_READ_MAXIMUM_BYTES,
                work: 1,
                operations: endpoint_request.maximum_operations,
            },
        );
        endpoint_capability.base_provider_generation = generation;
        // SAFETY: both identities name the actual original ownership and bounded
        // controller-configured endpoint; Source created neither permission.
        let ready = unsafe {
            arch::ReadyUsbProtocolHost::new(
                identity,
                endpoint_identity,
                controller,
                device,
                boot::executable_physical_address,
                endpoint.configured,
                endpoint.dma,
                endpoint.attachment,
            )
        }
        .map_err(|_| "protocol-usb-aggregate-identity-invalid")?;
        let source = arch::RdrandEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
        let mut entropy = CryptographicEntropyBase::<_, 2>::admit(source)
            .map_err(|_| "protocol-entropy-refused")?;
        // SAFETY: Root retained independent exact native authority for both calls.
        let control_issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
            .map_err(|_| "protocol-usb-issuance-refused")?;
        let endpoint_issuer =
            unsafe { NativeProtocolIssuer::admit(endpoint_capability, &mut entropy) }
                .map_err(|_| "protocol-usb-endpoint-issuance-refused")?;
        Ok(NativeUsbOwners::Protocol {
            ready: Box::new(ready),
            control_issuer: Box::new(control_issuer),
            endpoint_issuer: Box::new(endpoint_issuer),
        })
    } else {
        // SAFETY: actual controller/attachment are retained exclusively under
        // the exact control identity; mapping is checked by the constructor.
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
        let mut entropy = CryptographicEntropyBase::<_, 1>::admit(source)
            .map_err(|_| "protocol-entropy-refused")?;
        // SAFETY: retained native facts bind this exact finite control authority.
        let issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
            .map_err(|_| "protocol-usb-issuance-refused")?;
        Ok(NativeUsbOwners::Control {
            ready: Box::new(ready),
            issuer: Box::new(issuer),
        })
    }
}

/// Preserve the exact bounded native refusal; never guess a different port or
/// widen the administrative request to recover from an absent attachment.
fn native_refusal(stage: &str, reason: &'static str, root_port: u8, epoch: u32) -> &'static str {
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        use core::fmt::Write;
        let mut text = crate::sign_format::FixedText::new();
        let _ = writeln!(
            text,
            "CONDUIT_PROTOCOL_USB_REFUSAL {{\"schema\":\"conduit.conduitos/usb-root-refusal@1\",\"stage\":\"{stage}\",\"reason\":\"{reason}\",\"root_port\":{root_port},\"attachment_epoch\":{epoch}}}"
        );
        arch::early_write(text.as_bytes());
    }
    #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
    let _ = (stage, root_port, epoch);
    reason
}
