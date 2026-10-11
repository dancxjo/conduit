//! One exclusively reserved native USB attachment; no class policy.
use super::{
    authority::{CLAIMED, OperationAuthority, authority},
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

#[path = "usb_duplex_resources.rs"]
mod duplex_resources;
#[path = "usb_endpoint_owners.rs"]
mod endpoint;
#[path = "usb_isochronous_owners.rs"]
mod isochronous;
#[path = "usb_endpoint_write_owners.rs"]
mod outbound;

pub(super) enum NativeUsbOwners {
    Isochronous {
        ready: Box<arch::ReadyUsbIsochronousHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        isochronous_issuer: Box<NativeProtocolIssuer>,
    },
    Duplex {
        ready: Box<arch::ReadyUsbDuplexHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        read_issuer: Box<NativeProtocolIssuer>,
        write_issuer: Box<NativeProtocolIssuer>,
    },
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
    package: &[u8],
) -> Result<
    (
        NativeUsbOwners,
        Option<Box<super::usb_preflight::RetiredUsbDiscovery>>,
    ),
    &'static str,
> {
    #[cfg(not(feature = "native-owner-network"))]
    if request.ethernet_network.is_some() {
        return Err("protocol-usb-network-unsupported");
    }
    if request.endpoint_write.is_some() && request.endpoint_read.is_none() {
        return Err("protocol-usb-write-requires-read");
    }
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "protocol-resources-already-reserved")?;
    #[cfg(target_arch = "x86_64")]
    let initialized = arch::initialize_xhci(record.hhdm_offset, boot::executable_physical_address);
    #[cfg(target_arch = "aarch64")]
    // SAFETY: the sole Root retained the mapped ECAM/BAR and coherent DMA domain.
    let initialized =
        unsafe { arch::initialize_xhci(record.hhdm_offset, boot::executable_physical_address) };
    let mut controller = initialized.map_err(|error| {
        native_refusal(
            "controller",
            error.as_str(),
            request.root_port,
            request.attachment_epoch,
        )
    })?;
    let mut device = arch::address_selected_root_port_at_epoch(
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
    // Establish exactly once at the original address-completion boundary for
    // every Root route. Later discovery/Ready/native effects never clear CSC.
    let baseline = arch::AttachmentBaseline::establish(&controller, &device).map_err(|error| {
        native_refusal(
            "attachment-baseline",
            error.as_str(),
            device.root_port,
            device.attachment_epoch,
        )
    })?;
    // No class Ready, offer, grant or endpoint configuration exists until this
    // exact packaged pure predicate accepts the Root-observed attachment.
    let discovery = if let Some(preflight) = request.preflight.as_ref() {
        let snapshot = arch::discover_configuration(
            &mut controller,
            &mut device,
            boot::executable_physical_address,
            baseline,
        )
        .map_err(|error| {
            native_refusal(
                "discovery",
                error.as_str(),
                device.root_port,
                device.attachment_epoch,
            )
        })?;
        let admitted =
            super::usb_preflight::accept(package, request, preflight, snapshot, host, boot_id)?;
        admitted.validate_request(request)?;
        admitted.validate_attachment(&controller, &device)?;
        if !admitted.supported() {
            // One permanently claimed Boot Root retains the rejected pure
            // biography. This Box was reserved before Play; no retry/regrant.
            let _retained = Box::leak(admitted);
            return Err("protocol-usb-profile-unsupported");
        }
        Some(admitted)
    } else {
        baseline
            .validate_native(&controller, &device)
            .map_err(|error| {
                native_refusal(
                    "attachment-continuity",
                    error.as_str(),
                    device.root_port,
                    device.attachment_epoch,
                )
            })?;
        None
    };
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
    if let Some(iso_request) = request.isochronous_out.as_ref() {
        use crate::usb_base::isochronous::{contract::*, factory::*};
        let endpoint = unsafe { isochronous::configure(&mut controller, &device, iso_request) }?;
        let iso_identity = UsbNativeIdentity {
            host_id: host.clone(),
            boot_id: boot_id.clone(),
            base_id: format!("{prefix}/iso-controller").into(),
            provider_instance_id: format!("{prefix}/iso-provider").into(),
            provider_generation: generation,
            resource_pool_id: format!("{prefix}/iso-dma/{}", iso_request.endpoint_epoch).into(),
            resource_generation_id: ResourceGenerationId(format!(
                "{prefix}/iso-dma/{generation}/{}",
                iso_request.endpoint_epoch
            )),
            envelope_id: format!("{prefix}/iso-envelope/{}", iso_request.endpoint_epoch).into(),
            artifact_id: artifact.into(),
        };
        let mut iso_capability = authority(
            host,
            boot_id,
            "usb-isochronous-out",
            (
                &iso_identity.provider_instance_id,
                &iso_identity.resource_pool_id,
                &iso_identity.resource_generation_id,
                &iso_identity.envelope_id,
            ),
            OperationAuthority {
                contract: ISOCHRONOUS_OUT_AUTHORITY,
                call: ISOCHRONOUS_OUT_CALL,
                kind: ISOCHRONOUS_OUT_KIND,
                capability: ISOCHRONOUS_OUT_IMPLEMENTATION,
                bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES,
                work: 8,
                operations: iso_request.maximum_operations,
            },
        );
        iso_capability.base_provider_generation = generation;
        let ready = unsafe {
            arch::ReadyUsbIsochronousHost::new(
                identity,
                iso_identity,
                controller,
                device,
                boot::executable_physical_address,
                endpoint.configured,
                endpoint.dma,
                endpoint.attachment,
            )
        }
        .map_err(|_| "protocol-usb-iso-identity-invalid")?;
        let source = arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
        let mut entropy = CryptographicEntropyBase::<_, 2>::admit(source)
            .map_err(|_| "protocol-entropy-refused")?;
        let control_issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
            .map_err(|_| "protocol-usb-issuance-refused")?;
        let isochronous_issuer =
            unsafe { NativeProtocolIssuer::admit(iso_capability, &mut entropy) }
                .map_err(|_| "protocol-usb-iso-issuance-refused")?;
        return Ok((
            NativeUsbOwners::Isochronous {
                ready: Box::new(ready),
                control_issuer: Box::new(control_issuer),
                isochronous_issuer: Box::new(isochronous_issuer),
            },
            discovery,
        ));
    }
    if let Some(endpoint_request) = request.endpoint_read.as_ref() {
        // SAFETY: the sole Root reservation and administrator parameters bind
        // this exact controller/attachment; static DMA remains quarantined.
        let mut endpoint =
            unsafe { endpoint::configure(&mut controller, &device, endpoint_request) }?;
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
        if let Some(write_request) = request.endpoint_write.as_ref() {
            use crate::usb_base::{endpoint_write_contract::*, endpoint_write_factory::*};
            // SAFETY: the same reserved controller configures separate rooted OUT
            // storage using only the exact independently approved parameters.
            let mut write =
                unsafe { outbound::configure(&mut controller, &device, write_request) }?;
            let capacities = duplex_resources::admit(
                crate::allocation::BOOT_ARENA.capacity(),
                crate::allocation::BOOT_ARENA.live_bytes(),
                [
                    endpoint.attachment.resource_bytes,
                    write.attachment.resource_bytes,
                ],
            )?;
            endpoint.attachment.resource_bytes = capacities[0];
            write.attachment.resource_bytes = capacities[1];
            let write_identity = UsbNativeIdentity {
                host_id: host.clone(),
                boot_id: boot_id.clone(),
                base_id: format!("{prefix}/write-controller").into(),
                provider_instance_id: format!("{prefix}/write-provider").into(),
                provider_generation: generation,
                resource_pool_id: format!("{prefix}/write-dma/{}", write_request.endpoint_epoch)
                    .into(),
                resource_generation_id: ResourceGenerationId(format!(
                    "{prefix}/write-dma/{generation}/{}",
                    write_request.endpoint_epoch
                )),
                envelope_id: format!("{prefix}/write-envelope/{}", write_request.endpoint_epoch)
                    .into(),
                artifact_id: artifact.into(),
            };
            let mut write_capability = authority(
                host,
                boot_id,
                "usb-endpoint-write",
                (
                    &write_identity.provider_instance_id,
                    &write_identity.resource_pool_id,
                    &write_identity.resource_generation_id,
                    &write_identity.envelope_id,
                ),
                OperationAuthority {
                    contract: ENDPOINT_WRITE_AUTHORITY,
                    call: ENDPOINT_WRITE_CALL,
                    kind: ENDPOINT_WRITE_KIND,
                    capability: ENDPOINT_WRITE_IMPLEMENTATION,
                    bytes: ENDPOINT_WRITE_MAXIMUM_BYTES,
                    work: 1,
                    operations: write_request.maximum_operations,
                },
            );
            write_capability.base_provider_generation = generation;
            // SAFETY: the original attachment plus both real configured receipts
            // and disjoint rooted DMA are consumed by one sole native Host.
            let ready = unsafe {
                arch::ReadyUsbDuplexHost::new(
                    identity,
                    endpoint_identity,
                    write_identity,
                    controller,
                    device,
                    boot::executable_physical_address,
                    endpoint.configured,
                    endpoint.dma,
                    endpoint.attachment,
                    write.configured,
                    write.dma,
                    write.attachment,
                )
            }
            .map_err(|_| "protocol-usb-duplex-identity-invalid")?;
            let source =
                arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
            let mut entropy = CryptographicEntropyBase::<_, 3>::admit(source)
                .map_err(|_| "protocol-entropy-refused")?;
            // SAFETY: each issuer receives its independent bounded Root authority.
            let control_issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
                .map_err(|_| "protocol-usb-issuance-refused")?;
            let read_issuer =
                unsafe { NativeProtocolIssuer::admit(endpoint_capability, &mut entropy) }
                    .map_err(|_| "protocol-usb-endpoint-issuance-refused")?;
            let write_issuer =
                unsafe { NativeProtocolIssuer::admit(write_capability, &mut entropy) }
                    .map_err(|_| "protocol-usb-write-issuance-refused")?;
            return Ok((
                NativeUsbOwners::Duplex {
                    ready: Box::new(ready),
                    control_issuer: Box::new(control_issuer),
                    read_issuer: Box::new(read_issuer),
                    write_issuer: Box::new(write_issuer),
                },
                discovery,
            ));
        }
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
        let source = arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
        let mut entropy = CryptographicEntropyBase::<_, 2>::admit(source)
            .map_err(|_| "protocol-entropy-refused")?;
        // SAFETY: Root retained independent exact native authority for both calls.
        let control_issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
            .map_err(|_| "protocol-usb-issuance-refused")?;
        let endpoint_issuer =
            unsafe { NativeProtocolIssuer::admit(endpoint_capability, &mut entropy) }
                .map_err(|_| "protocol-usb-endpoint-issuance-refused")?;
        Ok((
            NativeUsbOwners::Protocol {
                ready: Box::new(ready),
                control_issuer: Box::new(control_issuer),
                endpoint_issuer: Box::new(endpoint_issuer),
            },
            discovery,
        ))
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
        let source = arch::DomainEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
        let mut entropy = CryptographicEntropyBase::<_, 1>::admit(source)
            .map_err(|_| "protocol-entropy-refused")?;
        // SAFETY: retained native facts bind this exact finite control authority.
        let issuer = unsafe { NativeProtocolIssuer::admit(capability, &mut entropy) }
            .map_err(|_| "protocol-usb-issuance-refused")?;
        Ok((
            NativeUsbOwners::Control {
                ready: Box::new(ready),
                issuer: Box::new(issuer),
            },
            discovery,
        ))
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
